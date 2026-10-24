//! Private owned LLVM consumer. All metadata is bound to one immutable witness
//! by ExecutionPlan; the scalar source route does not enter this module.
use super::super::native::hosted_frames;
use super::{plan::ExecutionPlan, verified::VerifiedOwnedProgram, *};
use crate::frontend::build_config::NativeSourceNames;
use plan::native_storage::{NativeFunctionStorage, NativeStoragePlan};
use std::fmt::Write;
use std::mem::size_of;

#[path = "native_bitwise.rs"]
mod bitwise;
#[path = "native_experimental.rs"]
mod experimental;
#[path = "native_floats.rs"]
mod floats;
#[path = "native_integer128.rs"]
mod integer128;
#[path = "native_integers.rs"]
mod integers;
#[path = "native_numeric.rs"]
mod numeric;
#[path = "native_resources.rs"]
mod resources;

const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMS: usize = 64;
const MAX_FUNCTION_SLOTS: usize = 256;
const MAX_SLOTS: usize = 8_192;
const MAX_NATIVE_INVENTORY_ITEMS: usize = 8_192;
const MAX_NATIVE_OWNER_WIDTH: usize = 8_192;
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
const INPUT_CAPACITY_INVARIANT: &str =
    "internal compiler error: owned execution invariant input capacity";
const OUTPUT_CAPACITY_INVARIANT: &str =
    "internal compiler error: owned execution invariant output capacity";
// Includes the fixed depth-bounded ScalarLeaves traversal stack; projection
// resolution and composite emission never retain an expanded leaf collection.
const EMITTER_TRANSIENT_BYTES: usize = 32_768;
// The borrowed native plan and its fixed checking/mapping carriers occupy this
// part of the inherited envelope; no retained table or new reserve is added.
const _: () = assert!(plan::native_storage::FIXED_CARRIER_ALLOWANCE <= EMITTER_TRANSIENT_BYTES);

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
    pub transfer_inventory_visits: usize,
    pub count_bytes: usize,
    pub render_bytes: usize,
    pub count_expansions: usize,
    pub count_expansion_kinds: [usize; 3],
    pub render_expansion_kinds: [usize; 3],
    pub render_expansions: usize,
    pub count_ordinary_visits: usize,
    pub render_ordinary_visits: usize,
    pub count_predecessor_visits: usize,
    pub render_predecessor_visits: usize,
    pub count_borrow_projection_visits: usize,
    pub render_borrow_projection_visits: usize,
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
    inventory_items: usize,
    owner_width: usize,
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
        inventory_items: MAX_NATIVE_INVENTORY_ITEMS,
        owner_width: MAX_NATIVE_OWNER_WIDTH,
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
            inventory_items: self.inventory_items.min(d.inventory_items),
            owner_width: self.owner_width.min(d.owner_width),
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
    // Unknown runtime cost: a CFG cycle, bounded input retries, or a callee
    // with either. Unrelated acyclic functions retain static fuel admission.
    cyclic: bool,
    depth: usize,
    scalar_slots: usize,
    // Common reference inventory retained for accounting observations, not
    // a native storage/admission limit. Logical fuel still uses the same X.
    cells: usize,
    bytes: usize,
}

/// Closed test control: only lower budgets or inject one new-reservation failure.
/// It cannot supply authority, choose an emitter, or expose the sealed witness.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct NativeControl {
    pub(super) fuel: usize,
    limits: Limits,
    fail_after: Option<usize>,
}
#[cfg(test)]
impl Default for NativeControl {
    fn default() -> Self {
        Self {
            fuel: plan::MAX_FUEL,
            limits: Limits::DEFAULT,
            fail_after: None,
        }
    }
}
#[cfg(test)]
pub(super) struct NativeObservation {
    pub(super) result: Result<String, Box<Diagnostic>>,
    pub(super) metrics: NativeMetrics,
}
#[cfg(test)]
pub(super) fn run_array_observed(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    control: NativeControl,
) -> NativeObservation {
    let mut accounting = Accounting {
        fail_after: control.fail_after,
        ..Accounting::default()
    };
    let result = native_module_accounted(
        witness,
        entry,
        sources,
        control.fuel,
        control.limits,
        &mut accounting,
    );
    NativeObservation {
        result,
        metrics: accounting.metrics,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeEntryPolicy {
    Result,
    Process,
    HostedResult,
    HostedProcess,
    TypedResult,
    HostedTypedResult,
    RecursiveHostedResult,
    RecursiveHostedProcess,
    RecursiveHostedTypedResult,
}

impl NativeEntryPolicy {
    fn process(self) -> bool {
        matches!(
            self,
            Self::Process | Self::HostedProcess | Self::RecursiveHostedProcess
        )
    }

    fn hosted(self) -> bool {
        matches!(
            self,
            Self::HostedResult
                | Self::HostedProcess
                | Self::HostedTypedResult
                | Self::RecursiveHostedResult
                | Self::RecursiveHostedProcess
                | Self::RecursiveHostedTypedResult
        )
    }

    fn recursive(self) -> bool {
        matches!(
            self,
            Self::RecursiveHostedResult
                | Self::RecursiveHostedProcess
                | Self::RecursiveHostedTypedResult
        )
    }

    fn with_recursion(self, recursive: bool) -> Self {
        match (self, recursive) {
            (Self::HostedResult, true) => Self::RecursiveHostedResult,
            (Self::HostedProcess, true) => Self::RecursiveHostedProcess,
            (Self::HostedTypedResult, true) => Self::RecursiveHostedTypedResult,
            _ => self,
        }
    }
}

pub(super) fn native_module_with_execution(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    process: bool,
    execution: crate::frontend::build_config::ExecutionPolicy,
    names: Option<&NativeSourceNames<'_>>,
) -> Result<String, Box<Diagnostic>> {
    let policy = match (process, execution.hosted()) {
        (false, false) => NativeEntryPolicy::TypedResult,
        (true, false) => NativeEntryPolicy::Process,
        (false, true) => NativeEntryPolicy::HostedTypedResult,
        (true, true) => NativeEntryPolicy::HostedProcess,
    };
    native_module_policy_accounted_with_source_names(
        witness,
        entry,
        sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        policy,
        &mut Accounting::default(),
        names,
    )
}

pub(super) fn native_process_module_with_fuel(
    witness: &VerifiedOwnedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    fuel: usize,
) -> Result<String, Box<Diagnostic>> {
    native_module_policy_accounted(
        witness,
        Some(entry),
        sources,
        fuel,
        Limits::DEFAULT,
        NativeEntryPolicy::Process,
        &mut Accounting::default(),
    )
}

pub(super) fn native_module(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
) -> Result<String, Box<Diagnostic>> {
    native_module_limits(witness, entry, sources, plan::MAX_FUEL, Limits::DEFAULT)
}

pub(super) fn native_module_with_source_names(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    names: &NativeSourceNames<'_>,
) -> Result<String, Box<Diagnostic>> {
    native_module_policy_accounted_with_source_names(
        witness,
        entry,
        sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        NativeEntryPolicy::Result,
        &mut Accounting::default(),
        Some(names),
    )
}

pub(super) fn native_process_module_with_source_names(
    witness: &VerifiedOwnedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    names: &NativeSourceNames<'_>,
) -> Result<String, Box<Diagnostic>> {
    native_module_policy_accounted_with_source_names(
        witness,
        Some(entry),
        sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        NativeEntryPolicy::Process,
        &mut Accounting::default(),
        Some(names),
    )
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
    native_module_policy_accounted(
        witness,
        entry,
        sources,
        fuel,
        limits,
        NativeEntryPolicy::Result,
        accounting,
    )
}
fn native_module_policy_accounted(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    fuel: usize,
    limits: Limits,
    policy: NativeEntryPolicy,
    accounting: &mut Accounting,
) -> Result<String, Box<Diagnostic>> {
    native_module_policy_accounted_with_source_names(
        witness, entry, sources, fuel, limits, policy, accounting, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn native_module_policy_accounted_with_source_names(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    fuel: usize,
    limits: Limits,
    policy: NativeEntryPolicy,
    accounting: &mut Accounting,
    names: Option<&NativeSourceNames<'_>>,
) -> Result<String, Box<Diagnostic>> {
    if let Some(names) = names {
        names.validate(sources)?;
    }
    // Inventory policy precedes plan allocation or emitted effects, including
    // unused output functions. Status-only declarations do not require it.
    if !policy.process() {
        for operation in [
            crate::frontend::builtin_catalog::experimental::Function::ReadSome,
            crate::frontend::builtin_catalog::experimental::Function::WriteSome,
        ] {
            if witness.experimental_function(operation).is_some() {
                return Err(reject("binary I/O requires process entry", None));
            }
        }
    }
    if !policy.process() && witness.builtin_output_function().is_some() {
        return Err(reject("write_stdout requires process entry", None));
    }
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
    if policy.process() && root.result != ValueTy::Scalar(hir::Ty::I32) {
        return Err(reject("process main must return i32", Some(root.span)));
    }
    if root.result == ValueTy::Scalar(hir::Ty::U8)
        && matches!(
            policy,
            NativeEntryPolicy::Result | NativeEntryPolicy::HostedResult
        )
    {
        return Err(reject(
            "native main must return bool, i32 or ()",
            Some(root.span),
        ));
    }
    let plan = ExecutionPlan::build(witness).map_err(|e| reject(e.name, e.span))?;
    accounting.metrics.plan_bytes = plan.metadata_bytes();
    accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(plan.metadata_bytes());
    let limits = limits.bounded();
    let bounds = admit_policy_accounted(&plan, limits, policy, accounting)?;
    let policy = policy.with_recursion(bounds.iter().any(|bound| bound.depth == 0));
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
    let guarded =
        plan.uses_dynamic_resources() || policy.process() || bounds.iter().any(|b| b.cyclic);
    let diagnostics = Diagnostics::new_policy_accounted_with_source_names(
        &plan,
        id,
        sources,
        guarded,
        policy,
        limits.diagnostic_bytes,
        limits.metadata_bytes,
        accounting,
        names,
    )?;
    let emitter_transient = add(
        add(
            EMITTER_TRANSIENT_BYTES,
            if policy.recursive() {
                hosted_frames::EMISSION_BYTES
            } else {
                0
            },
        )?,
        if plan.uses_dynamic_resources() {
            resources::control_bytes(&plan)
        } else {
            0
        },
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
        emitter_transient,
    )?;
    accounting.metrics.emitter_transient_bound = emitter_transient;
    accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(emission_metadata);
    limit(
        emission_metadata,
        limits.metadata_bytes,
        "emission metadata bytes",
        root.span,
    )?;
    let (transfer_cells, transfer_visits) = transfer_inventory(&plan)?;
    accounting.metrics.transfer_cells = transfer_cells;
    accounting.metrics.transfer_inventory_visits = transfer_visits;
    let storage = NativeStoragePlan::checked(&plan, guarded).map_err(|e| mismatch(e.name))?;
    let mut count = Emission {
        policy,
        ..Emission::count(limits.ir_bytes)
    };
    emit(
        &storage,
        id,
        &diagnostics,
        fuel.min(plan::MAX_FUEL),
        &mut count,
    );
    accounting.metrics.count_bytes = count.len;
    accounting.metrics.count_expansions = count.expansions;
    accounting.metrics.count_expansion_kinds = count.expansion_kinds;
    accounting.metrics.count_ordinary_visits = count.ordinary_visits;
    accounting.metrics.count_predecessor_visits = count.predecessor_visits;
    accounting.metrics.count_borrow_projection_visits = count.borrow_projection_visits;
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
    if count.expansions
        != add(
            accounting.metrics.transfer_cells,
            accounting.metrics.message_bytes,
        )?
    {
        return Err(mismatch("LLVM expansion inventory"));
    }
    let text = accounting.string(count.len, "LLVM")?;
    let mut output = Emission {
        text: Some(text),
        policy,
        ..Emission::count(count.len)
    };
    emit(
        &storage,
        id,
        &diagnostics,
        fuel.min(plan::MAX_FUEL),
        &mut output,
    );
    accounting.metrics.render_bytes = output.len;
    accounting.metrics.render_expansions = output.expansions;
    accounting.metrics.render_expansion_kinds = output.expansion_kinds;
    accounting.metrics.render_ordinary_visits = output.ordinary_visits;
    accounting.metrics.render_predecessor_visits = output.predecessor_visits;
    accounting.metrics.render_borrow_projection_visits = output.borrow_projection_visits;
    accounting.metrics.render_call_scratch_peak = output.call_scratch_peak;
    if output.exceeded
        || output.len != count.len
        || output.expansions != count.expansions
        || output.expansion_kinds != count.expansion_kinds
        || output.ordinary_visits != count.ordinary_visits
        || output.predecessor_visits != count.predecessor_visits
        || output.borrow_projection_visits != count.borrow_projection_visits
    {
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
    admit_policy_accounted(plan, limits, NativeEntryPolicy::Result, accounting)
}
fn admit_policy_accounted(
    plan: &ExecutionPlan<'_>,
    limits: Limits,
    policy: NativeEntryPolicy,
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
    let (mut scalar_slots, mut blocks, mut bytes) = (0, 0, 0);
    for f in functions {
        let u = plan.function(f.id).usage();
        limit(
            f.parameters.len(),
            limits.parameters,
            "parameter count",
            f.span,
        )?;
        // A slice contributes exactly one private i32 length beside its pointer.
        // Keep the source parameter limit unchanged while bounding ABI expansion.
        limit(
            add(f.parameters.len(), slice_reference_count(f))?,
            mul(limits.parameters, 2)?,
            "flattened parameter count",
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
    // Independent compiler inventories replace reference-runtime X here.
    // Fixed count facts end before graph, diagnostic and emission phases.
    admit_inventory_policy(plan, (limits.inventory_items, limits.owner_width))?;
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
        // dynamic callee makes the entire static sum nonbinding. Resource and
        // known acyclic arithmetic stay checked, while unknown sums saturate.
        let (local_cycle, cycle_scratch) = has_cycle(f)?;
        let scratch = add(
            add(fixed_scratch, mul(ready.capacity(), size_of::<usize>())?)?,
            cycle_scratch,
        )?;
        accounting.admission_peak(scratch)?;
        let cyclic = policy.hosted()
            || plan.uses_dynamic_resources()
            || local_cycle
            || plan.witness().builtin_function() == Some(f.id)
            || plan.witness().builtin_output_function() == Some(f.id)
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
            cost: u.activation_fuel_cells(),
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
                cost -= plan.function(target).usage().activation_fuel_cells();
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
        if !policy.hosted() {
            limit(bound.depth, limits.depth, "call depth", f.span)?;
        }
        limit(
            bound.scalar_slots,
            limits.scalar_slots,
            "live scalar slots",
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
    if policy.hosted() {
        // An unresolved call-cycle component is charged dynamically by its
        // callers. All ordinary per-function/compiler inventories still apply.
        for (i, pending) in remaining.iter().enumerate() {
            if *pending != 0 {
                let u = plan.function(functions[i].id).usage();
                bounds[i] = Bound {
                    cyclic: true,
                    scalar_slots: u.scalar_slots,
                    cells: u.expanded_cells,
                    bytes: u.native_bytes,
                    ..Bound::default()
                };
            }
        }
    } else if visited != functions.len() {
        let f = &functions[remaining
            .iter()
            .position(|&n| n != 0)
            .expect("cycle member")];
        return Err(reject("native preview does not support recursive call graphs, including unused functions and unchosen branches", Some(f.span)));
    }
    let recursive_policy = policy.with_recursion(bounds.iter().any(|bound| bound.depth == 0));
    let runtime_bytes = runtime_storage_bytes(
        plan,
        plan.uses_dynamic_resources() || policy.process() || bounds.iter().any(|b| b.cyclic),
        recursive_policy,
    )?;
    limit(
        add(bytes, runtime_bytes)?,
        limits.bytes,
        "aggregate storage bytes",
        origin,
    )?;
    for (f, bound) in functions.iter().zip(&bounds) {
        limit(
            add(bound.bytes, runtime_bytes)?,
            limits.live_bytes,
            "live storage bytes",
            f.span,
        )?;
    }
    Ok(bounds)
}
fn runtime_storage_bytes(
    plan: &ExecutionPlan<'_>,
    guarded: bool,
    policy: NativeEntryPolicy,
) -> Result<usize, Box<Diagnostic>> {
    let fuel_bytes = if guarded { 8 } else { 0 };
    let runtime_bytes = add(
        add(fuel_bytes, if plan.uses_floats() { 8192 } else { 0 })?,
        add(
            if plan.uses_dynamic_resources() { 64 } else { 0 },
            if plan.has_binary_delivery() {
                experimental::BINARY_DELIVERY_BYTES
            } else {
                0
            } + if plan.has_file_delivery() {
                experimental::FILE_DELIVERY_BYTES
            } else {
                0
            } + if plan.has_program_arguments() {
                experimental::ARGUMENT_DELIVERY_BYTES
            } else {
                0
            } + if plan.uses_wide_scalars() {
                experimental::WIDE_DELIVERY_BYTES
            } else {
                0
            },
        )?,
    )?;
    add(
        runtime_bytes,
        if policy.recursive() {
            hosted_frames::COUNTER_BYTES
        } else {
            0
        },
    )
}

fn successors(f: &RawOwnedFunction, kind: &OwnedTerminatorKind) -> impl Iterator<Item = BlockId> {
    let targets = match kind {
        OwnedTerminatorKind::MatchDispatch { match_id, arm } => {
            let descriptor = &f.matches[match_id.0];
            [
                Some(descriptor.arms[*arm].entry),
                descriptor.arms.get(arm + 1).map(|next| next.dispatch),
            ]
        }
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
        for target in successors(f, &b.terminator.as_ref().expect("verified terminator").kind) {
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
            f,
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

#[derive(Clone, Copy)]
enum Expansion {
    Construct = 0,
    Transfer = 1,
    Diagnostic = 2,
}
struct Emission {
    len: usize,
    text: Option<String>,
    maximum: usize,
    exceeded: bool,
    policy: NativeEntryPolicy,
    resources: bool,
    call_scratch_peak: usize,
    expansions: usize,
    expansion_kinds: [usize; 3],
    ordinary_visits: usize,
    predecessor_visits: usize,
    borrow_projection_visits: usize,
    #[cfg(test)]
    field_visits: usize,
}
impl Emission {
    fn expand(&mut self, kind: Expansion) -> bool {
        if self.exceeded {
            return false;
        }
        let next = self.expansions.checked_add(1);
        let ceiling = self.maximum.checked_add(1);
        match (next, ceiling) {
            (Some(next), Some(ceiling)) if next <= ceiling => {
                self.expansions = next;
                self.expansion_kinds[kind as usize] += 1;
                true
            }
            _ => {
                self.exceeded = true;
                false
            }
        }
    }
    fn count(maximum: usize) -> Self {
        Self {
            len: 0,
            text: None,
            maximum,
            exceeded: false,
            policy: NativeEntryPolicy::Result,
            resources: false,
            call_scratch_peak: 0,
            expansions: 0,
            expansion_kinds: [0; 3],
            ordinary_visits: 0,
            predecessor_visits: 0,
            borrow_projection_visits: 0,
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
// The private codebook keeps diagnostic keys at their existing one-byte size.
// Codes 0..16 retain the old ordering; extended codes are made only by the
// checked integer constructors below, with no producer-supplied numeric code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
struct FailureKind(u8);
const _: () = assert!(size_of::<FailureKind>() == 1);
#[allow(non_upper_case_globals)]
impl FailureKind {
    const Fuel: Self = Self(0);
    const Overflow: Self = Self(1);
    const DivisionByZero: Self = Self(2);
    const ByteRange: Self = Self(3);
    const Bounds: Self = Self(4);
    const EnumTag: Self = Self(5);
    const EnumPayload: Self = Self(6);
    const InputCapacity: Self = Self(7);
    const OutputCapacity: Self = Self(8);
    const ProcessStatus: Self = Self(9);
    const ResourceDescriptor: Self = Self(10);
    const AllocationIdentity: Self = Self(11);
    const WideOverflow: Self = Self(12);
    const WideDivisionByZero: Self = Self(13);
    const I64ToI32Range: Self = Self(14);
    const I64ToU8Range: Self = Self(15);
    const BitShift: Self = Self(16);
    const CharacterRange: Self = Self(17);
    const Frames: Self = Self(18);
    const Slots: Self = Self(19);
    const NativeFrameBytes: Self = Self(20);
    fn integer_code(ty: hir::Ty) -> u8 {
        match ty {
            hir::Ty::I8 => 0,
            hir::Ty::U8 => 1,
            hir::Ty::I16 => 2,
            hir::Ty::I32 => 3,
            hir::Ty::I64 => 4,
            hir::Ty::U16 => 5,
            hir::Ty::U32 => 6,
            hir::Ty::U64 => 7,
            hir::Ty::Isize => 8,
            hir::Ty::Usize => 9,
            hir::Ty::I128 => 10,
            hir::Ty::U128 => 11,
            hir::Ty::Char | hir::Ty::F32 | hir::Ty::F64 | hir::Ty::Bool | hir::Ty::Unit => {
                unreachable!("checked integer diagnostic")
            }
        }
    }
    fn integer_type(code: u8) -> hir::Ty {
        match code {
            0 => hir::Ty::I8,
            1 => hir::Ty::U8,
            2 => hir::Ty::I16,
            3 => hir::Ty::I32,
            4 => hir::Ty::I64,
            5 => hir::Ty::U16,
            6 => hir::Ty::U32,
            7 => hir::Ty::U64,
            8 => hir::Ty::Isize,
            9 => hir::Ty::Usize,
            10 => hir::Ty::I128,
            11 => hir::Ty::U128,
            _ => unreachable!("private integer diagnostic code"),
        }
    }
    fn integer_overflow(ty: hir::Ty) -> Self {
        Self(32 + Self::integer_code(ty))
    }
    fn integer_division_by_zero(ty: hir::Ty) -> Self {
        Self(48 + Self::integer_code(ty))
    }
    fn integer_range(input: hir::Ty, target: hir::Ty) -> Self {
        let input = Self::integer_code(input);
        let target = Self::integer_code(target);
        Self(match (input < 10, target < 10) {
            (true, true) => 64 + input * 10 + target,
            (false, true) => 164 + (input - 10) * 10 + target,
            (true, false) => 184 + input * 2 + target - 10,
            (false, false) => 204 + (input - 10) * 2 + target - 10,
        })
    }
    fn transient_bytes(self) -> usize {
        match self {
            Self::ResourceDescriptor => size_of::<Diagnostic>() + "internal compiler error: owned execution invariant resource descriptor".len(),
            Self::AllocationIdentity => size_of::<Diagnostic>() + "internal compiler error: owned execution invariant allocation identity exhausted".len(),
            Self::InputCapacity => size_of::<Diagnostic>() + INPUT_CAPACITY_INVARIANT.len(),
            Self::OutputCapacity => size_of::<Diagnostic>() + OUTPUT_CAPACITY_INVARIANT.len(),
            _ => DIAGNOSTIC_TRANSIENT_BYTES,
        }
    }
    fn diagnostic(self, span: Span, sources: &SourceMap) -> Box<Diagnostic> {
        match self {
            Self::AllocationIdentity | Self::ResourceDescriptor => Diagnostic::new(
                "E0500",
                "oir-owned-run",
                if self == Self::AllocationIdentity {
                    "internal compiler error: owned execution invariant allocation identity exhausted"
                } else {
                    "internal compiler error: owned execution invariant resource descriptor"
                },
                Some(span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::Fuel => RunFailure::Fuel(span).diagnostic(sources),
            Self::Frames => RunFailure::Frames(span).diagnostic(sources),
            Self::Slots => RunFailure::Slots(span).diagnostic(sources),
            Self::NativeFrameBytes => execute::OwnedRunFailure::Resource(
                plan::AdmissionFailure::new("live native frame bytes", Some(span)),
            )
            .diagnostic(sources),
            Self::Overflow => RunFailure::Overflow(span).diagnostic(sources),
            Self::DivisionByZero => RunFailure::DivisionByZero(span).diagnostic(sources),
            Self::ByteRange => RunFailure::ByteRange(span).diagnostic(sources),
            Self::BitShift => RunFailure::BitShift(span).diagnostic(sources),
            Self::CharacterRange => RunFailure::CharacterRange(span).diagnostic(sources),
            Self::WideOverflow => RunFailure::WideOverflow(span).diagnostic(sources),
            Self::WideDivisionByZero => RunFailure::WideDivisionByZero(span).diagnostic(sources),
            Self::I64ToI32Range => RunFailure::I64ToI32Range(span).diagnostic(sources),
            Self::I64ToU8Range => RunFailure::I64ToU8Range(span).diagnostic(sources),
            Self::Bounds => execute::OwnedRunFailure::Bounds(span).diagnostic(sources),
            Self::ProcessStatus => Diagnostic::new(
                "E0600",
                "oir-run",
                "process main must return a status in 0..255",
                Some(span).filter(|span| sources.is_valid_span(*span)),
            ),
            // Input capacity alone needs a 65-byte literal; inventory charges
            // that envelope only for modules that contain the input effect.
            Self::EnumTag | Self::EnumPayload | Self::InputCapacity | Self::OutputCapacity => {
                Diagnostic::new(
                    "E0500",
                    "oir-owned-run",
                    match self {
                        Self::EnumTag => {
                            "internal compiler error: owned execution invariant enum tag"
                        }
                        Self::EnumPayload => {
                            "internal compiler error: owned execution invariant enum payload"
                        }
                        Self::InputCapacity => INPUT_CAPACITY_INVARIANT,
                        Self::OutputCapacity => OUTPUT_CAPACITY_INVARIANT,
                        _ => unreachable!("enum invariant"),
                    },
                    Some(span).filter(|span| sources.is_valid_span(*span)),
                )
            }
            _ => match self.0 {
                32..=43 => RunFailure::IntegerOverflow {
                    ty: Self::integer_type(self.0 - 32),
                    span,
                }
                .diagnostic(sources),
                48..=59 => RunFailure::IntegerDivisionByZero {
                    ty: Self::integer_type(self.0 - 48),
                    span,
                }
                .diagnostic(sources),
                64..=163 => RunFailure::IntegerRange {
                    input: Self::integer_type((self.0 - 64) / 10),
                    target: Self::integer_type((self.0 - 64) % 10),
                    span,
                }
                .diagnostic(sources),
                164..=207 => {
                    let (input, target) = match self.0 {
                        164..=183 => ((self.0 - 164) / 10 + 10, (self.0 - 164) % 10),
                        184..=203 => ((self.0 - 184) / 2, (self.0 - 184) % 2 + 10),
                        _ => ((self.0 - 204) / 2 + 10, (self.0 - 204) % 2 + 10),
                    };
                    RunFailure::IntegerRange {
                        input: Self::integer_type(input),
                        target: Self::integer_type(target),
                        span,
                    }
                    .diagnostic(sources)
                }
                _ => unreachable!("private native diagnostic code"),
            },
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
        self.write_with_source_names(sources, None, out, accounting)
    }

    fn write_with_source_names(
        &self,
        sources: &SourceMap,
        names: Option<&NativeSourceNames<'_>>,
        out: &mut impl Write,
        accounting: &mut Accounting,
    ) -> std::fmt::Result {
        let span = self.span();
        let diagnostic = self.key.0.diagnostic(span, sources);
        let transient = size_of::<Diagnostic>() + diagnostic.message.capacity();
        accounting.metrics.diagnostic_transient_peak =
            accounting.metrics.diagnostic_transient_peak.max(transient);
        if transient > self.key.0.transient_bytes()
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
            let path = match names {
                Some(names) => names.path(span.file).ok_or(std::fmt::Error)?,
                None => sources.get(span.file).path(),
            };
            Ok((path, self.line, self.column))
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
    policy: NativeEntryPolicy,
    mut visit: impl FnMut(FailureKind, Span) -> Result<(), Box<Diagnostic>>,
) -> Result<(), Box<Diagnostic>> {
    if policy.process() {
        visit(
            FailureKind::ProcessStatus,
            plan.witness().functions()[entry.0].span,
        )?;
    }
    if guarded {
        visit(FailureKind::Fuel, plan.witness().functions()[entry.0].span)?;
    }
    if policy.recursive() {
        let span = plan.witness().functions()[entry.0].span;
        for kind in [
            FailureKind::Frames,
            FailureKind::Slots,
            FailureKind::NativeFrameBytes,
        ] {
            visit(kind, span)?;
        }
    }
    for f in plan.witness().functions() {
        if plan.uses_dynamic_resources() {
            visit(FailureKind::ResourceDescriptor, f.span)?;
            visit(FailureKind::EnumTag, f.span)?;
            visit(FailureKind::EnumPayload, f.span)?;
        }
        for parameter in &f.parameters {
            if let ParameterBinding::Owned(owner) = parameter {
                if matches!(f.owners[owner.0].aggregate(), AggregateTy::Enum(_)) {
                    visit(FailureKind::EnumTag, f.span)?;
                    visit(FailureKind::EnumPayload, f.span)?;
                }
            }
        }
        for b in &f.blocks {
            if guarded {
                if let Some(m) = &b.merge {
                    visit(FailureKind::Fuel, m.span)?;
                }
            }
            for statement in &b.statements {
                if matches!(statement.kind, OwnedInstruction::DestructureRecord { .. }) {
                    let span = plan::instruction_span(statement);
                    visit(FailureKind::ResourceDescriptor, span)?;
                    visit(FailureKind::EnumTag, span)?;
                    visit(FailureKind::EnumPayload, span)?;
                }
                if plan.uses_dynamic_resources() {
                    let span = plan::instruction_span(statement);
                    visit(FailureKind::ResourceDescriptor, span)?;
                    visit(FailureKind::EnumTag, span)?;
                    visit(FailureKind::EnumPayload, span)?;
                    if let OwnedInstruction::ConsumeOwnedVariant { match_id, .. } = statement.kind {
                        visit(FailureKind::ResourceDescriptor, f.matches[match_id.0].span)?;
                    }
                }
                if guarded {
                    visit(FailureKind::Fuel, plan::instruction_span(statement))?;
                }
                match &statement.kind {
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value:
                            Rvalue::CharacterConvert {
                                op: hir::ConversionOp::ToCharChecked,
                                name_span,
                                ..
                            },
                        ..
                    })) => visit(FailureKind::CharacterRange, *name_span)?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value:
                            Rvalue::Bitwise {
                                op, operator_span, ..
                            },
                        ..
                    })) if op.shift() => visit(FailureKind::BitShift, *operator_span)?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        destination,
                        value:
                            Rvalue::CheckedInteger {
                                op, operator_span, ..
                            },
                        ..
                    })) => {
                        let kind = f.locals[destination.0].ty;
                        if !matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder)
                            || kind.signed_integer()
                        {
                            visit(FailureKind::integer_overflow(kind), *operator_span)?;
                        }
                        if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
                            visit(FailureKind::integer_division_by_zero(kind), *operator_span)?;
                        }
                    }
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        destination,
                        value: Rvalue::CheckedNegateInteger { operator_span, .. },
                        ..
                    })) => visit(
                        FailureKind::integer_overflow(f.locals[destination.0].ty),
                        *operator_span,
                    )?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value:
                            Rvalue::CheckedIntegerCast {
                                operand,
                                target,
                                name_span,
                                ..
                            },
                        ..
                    })) => visit(
                        FailureKind::integer_range(f.locals[operand.local.0].ty, *target),
                        *name_span,
                    )?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value:
                            Rvalue::CheckedI64 {
                                op, operator_span, ..
                            },
                        ..
                    })) => {
                        visit(FailureKind::WideOverflow, *operator_span)?;
                        if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
                            visit(FailureKind::WideDivisionByZero, *operator_span)?;
                        }
                    }
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedNegateI64 { operator_span, .. },
                        ..
                    })) => visit(FailureKind::WideOverflow, *operator_span)?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI64ToI32 { name_span, .. },
                        ..
                    })) => visit(FailureKind::I64ToI32Range, *name_span)?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI64ToU8 { name_span, .. },
                        ..
                    })) => visit(FailureKind::I64ToU8Range, *name_span)?,
                    OwnedInstruction::Experimental { .. } => {
                        visit(FailureKind::Bounds, plan::instruction_span(statement))?;
                        visit(
                            FailureKind::AllocationIdentity,
                            plan::instruction_span(statement),
                        )?;
                    }
                    OwnedInstruction::PrepareBorrowRange { .. } => {
                        visit(FailureKind::Bounds, plan::instruction_span(statement))?
                    }
                    OwnedInstruction::PrepareBorrow { loan, .. }
                        if experimental::resource_borrow(plan, f, *loan) =>
                    {
                        visit(FailureKind::Bounds, plan::instruction_span(statement))?
                    }
                    OwnedInstruction::ReadStdin { .. } => visit(
                        FailureKind::InputCapacity,
                        plan::instruction_span(statement),
                    )?,
                    OwnedInstruction::WriteStdout { .. } => visit(
                        FailureKind::OutputCapacity,
                        plan::instruction_span(statement),
                    )?,
                    OwnedInstruction::ConsumeVariant { match_id, .. }
                    | OwnedInstruction::ConsumeOwnedVariant { match_id, .. } => {
                        let span = f.matches[match_id.0].span;
                        visit(FailureKind::EnumTag, span)?;
                        visit(FailureKind::EnumPayload, span)?;
                    }
                    OwnedInstruction::ConstructOwnedEnum { .. } => {
                        let span = plan::instruction_span(statement);
                        visit(FailureKind::EnumTag, span)?;
                        visit(FailureKind::EnumPayload, span)?;
                    }
                    OwnedInstruction::MoveInitialize { source, .. }
                    | OwnedInstruction::Replace { source, .. }
                    | OwnedInstruction::PrepareOwned { source, .. }
                    | OwnedInstruction::Discard(source)
                        if matches!(f.owners[source.0].aggregate(), AggregateTy::Enum(_)) =>
                    {
                        let span = plan::instruction_span(statement);
                        visit(FailureKind::EnumTag, span)?;
                        visit(FailureKind::EnumPayload, span)?;
                    }
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value:
                            Rvalue::CheckedI32 {
                                op, operator_span, ..
                            },
                        ..
                    })) => {
                        visit(FailureKind::Overflow, *operator_span)?;
                        if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
                            visit(FailureKind::DivisionByZero, *operator_span)?;
                        }
                    }
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI32ToU8 { name_span, .. },
                        ..
                    })) => visit(FailureKind::ByteRange, *name_span)?,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedNegateI32 { operator_span, .. },
                        ..
                    })) => visit(FailureKind::Overflow, *operator_span)?,
                    OwnedInstruction::ReadIndex { .. }
                    | OwnedInstruction::WriteIndex { .. }
                    | OwnedInstruction::ReadProjection { index: Some(_), .. }
                    | OwnedInstruction::WriteProjection { index: Some(_), .. } => {
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
            let term = b.terminator.as_ref().expect("verified terminator");
            if policy.recursive() && matches!(term.kind, OwnedTerminatorKind::Invoke { .. }) {
                for kind in [
                    FailureKind::Frames,
                    FailureKind::Slots,
                    FailureKind::NativeFrameBytes,
                ] {
                    visit(kind, term.span)?;
                }
            }
            if plan.uses_dynamic_resources() {
                visit(FailureKind::ResourceDescriptor, term.span)?;
                visit(FailureKind::EnumTag, term.span)?;
                visit(FailureKind::EnumPayload, term.span)?;
            }
            match term.kind {
                OwnedTerminatorKind::MatchDispatch { match_id, .. } => {
                    visit(FailureKind::EnumTag, f.matches[match_id.0].span)?
                }
                OwnedTerminatorKind::ReturnOwned(owner)
                    if matches!(f.owners[owner.0].aggregate(), AggregateTy::Enum(_)) =>
                {
                    visit(FailureKind::EnumTag, term.span)?;
                    visit(FailureKind::EnumPayload, term.span)?;
                }
                _ => {}
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
        Self::new_policy_accounted(
            plan,
            entry,
            sources,
            guarded,
            NativeEntryPolicy::Result,
            maximum,
            metadata_maximum,
            accounting,
        )
    }
    // Entry policy is explicit alongside the existing accounted inputs.
    #[allow(clippy::too_many_arguments)]
    fn new_policy_accounted(
        plan: &ExecutionPlan<'_>,
        entry: hir::DefId,
        sources: &SourceMap,
        guarded: bool,
        policy: NativeEntryPolicy,
        maximum: usize,
        metadata_maximum: usize,
        accounting: &mut Accounting,
    ) -> Result<Self, Box<Diagnostic>> {
        Self::new_policy_accounted_with_source_names(
            plan,
            entry,
            sources,
            guarded,
            policy,
            maximum,
            metadata_maximum,
            accounting,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_policy_accounted_with_source_names(
        plan: &ExecutionPlan<'_>,
        entry: hir::DefId,
        sources: &SourceMap,
        guarded: bool,
        policy: NativeEntryPolicy,
        maximum: usize,
        metadata_maximum: usize,
        accounting: &mut Accounting,
        names: Option<&NativeSourceNames<'_>>,
    ) -> Result<Self, Box<Diagnostic>> {
        let mut count = 0;
        let mut diagnostic_transient = DIAGNOSTIC_TRANSIENT_BYTES;
        diagnostic_occurrences(plan, entry, guarded, policy, |kind, _| {
            count = add(count, 1)?;
            diagnostic_transient = diagnostic_transient.max(kind.transient_bytes());
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
                diagnostic_transient,
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
        diagnostic_occurrences(plan, entry, guarded, policy, |kind, span| {
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
            add(metadata, diagnostic_transient)?,
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
            let result = row.write_with_source_names(sources, names, &mut bytes, accounting);
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
            row.write_with_source_names(sources, names, &mut output, accounting)
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
    if out.resources {
        out.write_str("  call void @__oxid_bytebuf_cleanup()\n")
            .unwrap();
    }
    let (id, len) = diagnostics.get(kind, span);
    let helper = match out.policy {
        NativeEntryPolicy::Result
        | NativeEntryPolicy::HostedResult
        | NativeEntryPolicy::TypedResult
        | NativeEntryPolicy::HostedTypedResult
        | NativeEntryPolicy::RecursiveHostedResult
        | NativeEntryPolicy::RecursiveHostedTypedResult => "__oxid_overflow",
        NativeEntryPolicy::Process
        | NativeEntryPolicy::HostedProcess
        | NativeEntryPolicy::RecursiveHostedProcess => "__oxid_process_failure",
    };
    writeln!(
        out,
        "  call void @{helper}(ptr @__oxid_owned_error_{id}, i64 {len})\n  unreachable"
    )
    .unwrap();
}
fn emit_guard(out: &mut Emission, diagnostics: &Diagnostics, name: &str, cost: usize, span: Span) {
    emit_guard_value(out, diagnostics, name, cost, span);
}
fn emit_guard_value(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    cost: impl std::fmt::Display,
    span: Span,
) {
    if out.policy.hosted() {
        // Keep the continuation identity used by phi/call predecessor maps.
        // Hosted execution removes the counter and failure edge, not the
        // verified control-flow relationship between adjacent operations.
        writeln!(out, "  br label %{name}_ok\n{name}_ok:").unwrap();
        return;
    }
    writeln!(out, "  %{name}_remaining = load i64, ptr %fuel\n  %{name}_exhausted = icmp ult i64 %{name}_remaining, {cost}\n  br i1 %{name}_exhausted, label %{name}_error, label %{name}_ok\n{name}_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::Fuel, span);
    writeln!(out, "{name}_ok:\n  %{name}_next = sub i64 %{name}_remaining, {cost}\n  store i64 %{name}_next, ptr %fuel").unwrap();
}
fn ty(t: hir::Ty) -> &'static str {
    match t {
        hir::Ty::F32 => "float",
        hir::Ty::F64 => "double",
        hir::Ty::Bool => "i1",
        hir::Ty::Char | hir::Ty::I32 | hir::Ty::U32 => "i32",
        hir::Ty::I16 | hir::Ty::U16 => "i16",
        hir::Ty::I64 | hir::Ty::U64 | hir::Ty::Isize | hir::Ty::Usize => "i64",
        hir::Ty::I128 | hir::Ty::U128 => "i128",
        hir::Ty::I8 | hir::Ty::U8 | hir::Ty::Unit => "i8",
    }
}
fn result_ty(t: ValueTy) -> &'static str {
    match t {
        ValueTy::Scalar(t) => ty(t),
        ValueTy::Owned(_) => "void",
    }
}
fn load_slot(out: &mut Emission, name: &str, pointer: &str, t: hir::Ty) -> String {
    if t.floating() {
        writeln!(out, "  %{name} = load {}, ptr {pointer}, align 8", ty(t)).unwrap();
        return format!("%{name}");
    }
    if t.quad_integer() {
        writeln!(out, "  %{name} = load i128, ptr {pointer}, align 16").unwrap();
        return format!("%{name}");
    }
    if t.integer_bits() == Some(64) {
        writeln!(out, "  %{name} = load i64, ptr {pointer}, align 8").unwrap();
        return format!("%{name}");
    }
    writeln!(out, "  %{name}_wide = load i64, ptr {pointer}, align 8\n  %{name} = trunc i64 %{name}_wide to {}", ty(t)).unwrap();
    format!("%{name}")
}
fn store_slot(out: &mut Emission, name: &str, pointer: &str, t: hir::Ty, value: &str) {
    if t.floating() {
        writeln!(out, "  store {} {value}, ptr {pointer}, align 8", ty(t)).unwrap();
        return;
    }
    if t.quad_integer() {
        writeln!(out, "  store i128 {value}, ptr {pointer}, align 16").unwrap();
        return;
    }
    if t.integer_bits() == Some(64) {
        writeln!(out, "  store i64 {value}, ptr {pointer}, align 8").unwrap();
        return;
    }
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
fn slice_reference_count(f: &RawOwnedFunction) -> usize {
    f.references
        .iter()
        .filter(|reference| matches!(reference.referent(), BorrowedTy::ScalarSlice(_)))
        .count()
}
fn indexed_base(f: &RawOwnedFunction, base: AccessBase) -> BorrowedTy {
    match base {
        AccessBase::Owner(owner) => BorrowedTy::Exact(f.owners[owner.0].aggregate()),
        AccessBase::Parameter(reference) => f.references[reference.0].referent(),
    }
}
fn index_length(out: &mut Emission, f: &RawOwnedFunction, name: &str, base: AccessBase) -> String {
    match indexed_base(f, base) {
        BorrowedTy::Exact(AggregateTy::FixedArray(array)) => array.length().to_string(),
        BorrowedTy::ScalarSlice(_) => {
            let AccessBase::Parameter(reference) = base else {
                unreachable!("slices are borrowed parameter views")
            };
            writeln!(
                out,
                "  %{name}_length = load i32, ptr %rl{}, align 4",
                reference.0
            )
            .unwrap();
            format!("%{name}_length")
        }
        BorrowedTy::Exact(
            AggregateTy::Record(_)
            | AggregateTy::Enum(_)
            | AggregateTy::ByteBuf
            | AggregateTy::File,
        ) => {
            unreachable!("verified indexed base")
        }
    }
}
fn element_stride(element: hir::Ty) -> usize {
    match element {
        hir::Ty::I16 | hir::Ty::U16 => 2,
        hir::Ty::Char | hir::Ty::F32 | hir::Ty::I32 | hir::Ty::U32 => 4,
        hir::Ty::F64 | hir::Ty::I64 | hir::Ty::U64 | hir::Ty::Isize | hir::Ty::Usize => 8,
        hir::Ty::I128 | hir::Ty::U128 => 16,
        hir::Ty::I8 | hir::Ty::Bool | hir::Ty::U8 | hir::Ty::Unit => 1,
    }
}
fn sentinel_ty(array: FixedArrayTy) -> &'static str {
    match array.element() {
        hir::Ty::I16 | hir::Ty::U16 => "i16",
        hir::Ty::Char | hir::Ty::F32 | hir::Ty::I32 | hir::Ty::U32 => "i32",
        hir::Ty::F64 | hir::Ty::I64 | hir::Ty::U64 | hir::Ty::Isize | hir::Ty::Usize => "i64",
        hir::Ty::I128 | hir::Ty::U128 => "i128",
        hir::Ty::I8 | hir::Ty::Bool | hir::Ty::U8 | hir::Ty::Unit => "i8",
    }
}
/// Resolve from the verified nominal root every time; raw offsets are never
/// authority, and this bounded path walk allocates no projection metadata.
fn projection(
    plan: &ExecutionPlan<'_>,
    f: &RawOwnedFunction,
    base: AccessBase,
    path: &[FieldId],
) -> (ValueTy, usize) {
    let BorrowedTy::Exact(root) = indexed_base(f, base) else {
        unreachable!("verified record projection root")
    };
    plan.witness()
        .declarations()
        .projection(root, path)
        .expect("verified projection")
}

fn index_pointer(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    name: &str,
    statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) -> (hir::Ty, String) {
    let f = &plan.witness().functions()[id.0];
    let (base, index, element, length, projection_offset) = match &statement.kind {
        OwnedInstruction::ReadIndex { base, index, .. }
        | OwnedInstruction::WriteIndex { base, index, .. } => (
            *base,
            *index,
            indexed_base(f, *base)
                .element()
                .expect("verified indexed base"),
            index_length(out, f, name, *base),
            None,
        ),
        OwnedInstruction::ReadProjection {
            base,
            path,
            index: Some(index),
            ..
        }
        | OwnedInstruction::WriteProjection {
            base,
            path,
            index: Some(index),
            ..
        } => {
            let (value, offset) = projection(plan, f, *base, path);
            let ValueTy::Owned(AggregateTy::FixedArray(array)) = value else {
                unreachable!("verified projected indexed base")
            };
            (
                *base,
                *index,
                array.element(),
                array.length().to_string(),
                Some(offset),
            )
        }
        _ => unreachable!("indexed operation"),
    };
    let index = load_operand(out, f, &format!("{name}_index"), index);
    let suffix = continuation(f, &statement.kind)
        .suffix()
        .expect("indexed continuation");
    writeln!(out, "  %{name}_nonnegative = icmp sge i32 {index}, 0\n  %{name}_below = icmp slt i32 {index}, {}\n  %{name}_in_range = and i1 %{name}_nonnegative, %{name}_below\n  br i1 %{name}_in_range, label %{name}_{suffix}, label %{name}_bounds_error\n{name}_bounds_error:", length).unwrap();
    emit_failure(
        out,
        diagnostics,
        FailureKind::Bounds,
        plan::instruction_span(statement),
    );
    writeln!(out, "{name}_{suffix}:\n  %{name}_index64 = zext i32 {index} to i64\n  %{name}_offset = mul i64 %{name}_index64, {}", element_stride(element)).unwrap();
    // Even resolving a reference base happens only on the successful edge.
    let base = base_pointer(out, name, base);
    let base = match projection_offset {
        Some(offset) => field_pointer(out, &format!("{name}_projection"), &base, offset),
        None => base,
    };
    writeln!(
        out,
        "  %{name}_ptr = getelementptr i8, ptr {base}, i64 %{name}_offset"
    )
    .unwrap();
    (element, format!("%{name}_ptr"))
}

/// Counts raw sites and active enum branch trees without expanding static
/// record or array leaves during inventory.
fn transfer_inventory(plan: &ExecutionPlan<'_>) -> Result<(usize, usize), Box<Diagnostic>> {
    let mut cells = 0;
    let mut visits = 0;
    for f in plan.witness().functions() {
        visits = add(visits, 1)?;
        for parameter in &f.parameters {
            visits = add(visits, 1)?;
            if let ParameterBinding::Owned(owner) = parameter {
                cells = add(
                    cells,
                    transfer_expansions_counted(plan, f.id, *owner, &mut visits)?,
                )?;
            }
        }
        for block in &f.blocks {
            visits = add(visits, 1)?;
            for statement in &block.statements {
                visits = add(visits, 1)?;
                let owner = match statement.kind {
                    OwnedInstruction::ReadStdin { .. } => {
                        // Fixed result branches: Eof tag/payload, Full tag,
                        // IoError tag. The bounded commit loop is fixed text,
                        // not a capacity-sized scalar expansion.
                        cells = add(cells, 4)?;
                        None
                    }
                    OwnedInstruction::WriteStdout { .. } => {
                        // Complete/InvalidInput tags and IoError tag/payload.
                        // Staging and one-byte attempts remain fixed loop text.
                        cells = add(cells, 4)?;
                        None
                    }
                    OwnedInstruction::DestructureRecord { ref fields, .. } => {
                        cells = add(cells, 1)?;
                        for (_, destination) in fields {
                            visits = add(visits, 1)?;
                            let aggregate = f.owners[destination.0].aggregate();
                            cells = add(
                                cells,
                                active_transfer_inventory(plan, aggregate, false, &mut visits)?,
                            )?;
                            cells = add(
                                cells,
                                active_transfer_inventory(plan, aggregate, true, &mut visits)?,
                            )?;
                        }
                        None
                    }
                    OwnedInstruction::ConstructEnum { payload, .. } => {
                        cells = add(cells, 1 + usize::from(payload.is_some()))?;
                        None
                    }
                    OwnedInstruction::ConstructOwnedEnum { payload, .. } => {
                        cells = add(
                            cells,
                            add(
                                1,
                                active_transfer_inventory(
                                    plan,
                                    f.owners[payload.0].aggregate(),
                                    true,
                                    &mut visits,
                                )?,
                            )?,
                        )?;
                        None
                    }
                    OwnedInstruction::ConsumeOwnedVariant { destination, .. } => {
                        cells = add(
                            cells,
                            active_transfer_inventory(
                                plan,
                                f.owners[destination.0].aggregate(),
                                true,
                                &mut visits,
                            )?,
                        )?;
                        None
                    }
                    OwnedInstruction::Discard(owner)
                        if matches!(f.owners[owner.0].aggregate(), AggregateTy::Enum(_)) =>
                    {
                        cells = add(
                            cells,
                            active_transfer_inventory(
                                plan,
                                f.owners[owner.0].aggregate(),
                                false,
                                &mut visits,
                            )?,
                        )?;
                        None
                    }
                    OwnedInstruction::Construct { destination, .. }
                    | OwnedInstruction::ConstructComposite { destination, .. }
                    | OwnedInstruction::ConstructArray { destination, .. } => {
                        // Construction writes each logical leaf once. Resource
                        // sources have a separate complete preflight below;
                        // this is not a whole-owner checked move.
                        cells = add(cells, plan.owner_width(f.id, destination))?;
                        None
                    }
                    OwnedInstruction::MoveInitialize { source, .. }
                    | OwnedInstruction::Replace { source, .. }
                    | OwnedInstruction::PrepareOwned { source, .. } => Some(source),
                    _ => None,
                };
                if let Some(owner) = owner {
                    cells = add(
                        cells,
                        transfer_expansions_counted(plan, f.id, owner, &mut visits)?,
                    )?;
                }
            }
            visits = add(visits, 1)?;
            if let OwnedTerminatorKind::ReturnOwned(owner) =
                block.terminator.as_ref().expect("verified terminator").kind
            {
                // The sealed binary adapter already commits the status to
                // the caller; its immediate return emits no transfer tree.
                if plan.binary_delivery_cost(f.id).is_none()
                    && plan.file_delivery_cost(f.id).is_none()
                {
                    cells = add(
                        cells,
                        transfer_expansions_counted(plan, f.id, owner, &mut visits)?,
                    )?;
                }
            }
        }
    }
    // Resource preflight and release add no user fuel. All of their generated
    // tag/leaf checks still belong to the native text expansion inventory.
    for function in plan.witness().functions() {
        for block in &function.blocks {
            for statement in &block.statements {
                match &statement.kind {
                    OwnedInstruction::Replace { source, .. } => {
                        let aggregate = function.owners[source.0].aggregate();
                        if resources::contains(plan, aggregate) {
                            cells = add(
                                cells,
                                active_transfer_inventory(plan, aggregate, false, &mut visits)?,
                            )?;
                        }
                    }
                    OwnedInstruction::ConstructComposite {
                        destination,
                        fields,
                    } if resources::contains(plan, function.owners[destination.0].aggregate()) => {
                        for (_, value) in fields {
                            if let FieldInitializer::Owned(source) = value {
                                cells = add(
                                    cells,
                                    active_transfer_inventory(
                                        plan,
                                        function.owners[source.0].aggregate(),
                                        false,
                                        &mut visits,
                                    )?,
                                )?;
                            }
                        }
                    }
                    _ => {}
                }
                let owner = match statement.kind {
                    OwnedInstruction::StorageEnd(owner) => Some(owner),
                    OwnedInstruction::Replace { destination, .. } => Some(destination),
                    OwnedInstruction::Discard(owner)
                        if !matches!(
                            function.owners[owner.0].aggregate(),
                            AggregateTy::Enum(_)
                        ) =>
                    {
                        Some(owner)
                    }
                    _ => None,
                };
                if let Some(owner) = owner {
                    let aggregate = function.owners[owner.0].aggregate();
                    if resources::contains(plan, aggregate) {
                        cells = add(
                            cells,
                            active_transfer_inventory(plan, aggregate, false, &mut visits)?,
                        )?;
                    }
                }
            }
            if matches!(
                block.terminator.as_ref().expect("verified terminator").kind,
                OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_)
            ) && plan.file_delivery_cost(function.id).is_none()
            {
                for owner in &function.owners {
                    if resources::contains(plan, owner.aggregate()) {
                        cells = add(
                            cells,
                            active_transfer_inventory(plan, owner.aggregate(), false, &mut visits)?,
                        )?;
                    }
                }
            }
        }
    }
    Ok((cells, visits))
}
/// Tag-directed enum validation/copy work grows with the number of cases, not
/// logical owner width. The switch rows and bodies are separately expanded.
fn transfer_expansions(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    owner: OwnerPlaceId,
) -> Result<usize, Box<Diagnostic>> {
    transfer_expansions_counted(plan, id, owner, &mut 0)
}
fn transfer_expansions_counted(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    owner: OwnerPlaceId,
    visits: &mut usize,
) -> Result<usize, Box<Diagnostic>> {
    let aggregate = plan.witness().functions()[id.0].owners[owner.0].aggregate();
    if matches!(aggregate, AggregateTy::Enum(_)) || resources::contains(plan, aggregate) {
        active_transfer_inventory(plan, aggregate, true, visits)
    } else {
        Ok(plan.owner_width(id, owner))
    }
}

/// Count the same switch rows, case bodies, active validation cells and copy
/// cells as the emitter. The bounded stack retains no expanded variant tree.
/// Reject a count larger than the existing text ceiling before expanding it.
fn active_transfer_inventory(
    plan: &ExecutionPlan<'_>,
    aggregate: AggregateTy,
    copy: bool,
    visits: &mut usize,
) -> Result<usize, Box<Diagnostic>> {
    let declarations = plan.witness().declarations();
    let factor = 1 + usize::from(copy);
    let AggregateTy::Enum(enumeration) = aggregate else {
        *visits = add(*visits, 1)?;
        return mul(
            declarations
                .aggregate_width(aggregate)
                .expect("verified width"),
            factor,
        );
    };
    let variants = declarations
        .enums()
        .variants(enumeration)
        .expect("verified enum");
    if !variants
        .iter()
        .any(|variant| matches!(variant.value_payload(), Some(ValueTy::Owned(_))))
    {
        return mul(2, variants.len());
    }
    let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
    stack[0] = Some((enumeration, 0usize));
    let mut length = 1;
    let mut cells = 0;
    while length != 0 {
        let (enumeration, next) = stack[length - 1].expect("checked enum traversal");
        let variants = declarations
            .enums()
            .variants(enumeration)
            .expect("verified enum");
        if next == 0 {
            *visits = add(*visits, 1)?;
            cells = add(cells, mul(2, variants.len())?)?;
        }
        if let Some(variant) = variants.get(next) {
            *visits = add(*visits, 1)?;
            stack[length - 1] = Some((enumeration, next + 1));
            if let Some(ValueTy::Owned(payload)) = variant.value_payload() {
                match payload {
                    AggregateTy::Enum(child) => {
                        if length == stack.len() {
                            return Err(mismatch("enum transfer depth"));
                        }
                        stack[length] = Some((child, 0));
                        length += 1;
                    }
                    _ => {
                        cells = add(
                            cells,
                            mul(
                                declarations
                                    .aggregate_width(payload)
                                    .expect("verified payload width"),
                                factor,
                            )?,
                        )?
                    }
                }
            }
        } else {
            length -= 1;
        }
        if cells > MAX_IR_BYTES {
            return Err(reject(
                "native owned LLVM bytes limit exceeded by enum transfer expansion",
                None,
            ));
        }
    }
    Ok(cells)
}

/// All callers receive types, extents and ownership transitions from the sealed
/// witness. Runtime tags and active payload cells still require validation.
fn transfer_checked(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    pointers: (&str, &str),
    failure: (&Diagnostics, Span),
) {
    match aggregate {
        AggregateTy::Enum(enumeration) => enum_transfer(
            out,
            plan,
            name,
            enumeration,
            pointers.0,
            Some(pointers.1),
            failure,
        ),
        _ if resources::contains(plan, aggregate) => active_transfer(
            out,
            plan,
            name,
            aggregate,
            pointers.0,
            Some(pointers.1),
            failure,
        ),
        _ => transfer(out, plan, name, aggregate, pointers, failure),
    }
}

/// Read precisely the active scalar after the caller has proved the tag.
/// In particular an i1 load cannot establish a canonical boolean byte.
fn enum_payload_load(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    source: &str,
    variant: VariantId,
    failure: (&Diagnostics, Span),
) -> Option<(hir::Ty, String)> {
    let declaration = plan
        .witness()
        .declarations()
        .enums()
        .variant(variant.enumeration, variant)
        .expect("verified variant");
    let scalar = match declaration.value_payload()? {
        ValueTy::Scalar(scalar) => scalar,
        ValueTy::Owned(_) => unreachable!("owned payload requires active aggregate transfer"),
    };
    let pointer = field_pointer(
        out,
        &format!("{name}_active"),
        source,
        declaration.payload_offset().expect("active scalar offset"),
    );
    let storage = if matches!(scalar, hir::Ty::Bool | hir::Ty::Unit) {
        "i8"
    } else {
        ty(scalar)
    };
    writeln!(
        out,
        "  %{name}_active = load {storage}, ptr {pointer}, align 1"
    )
    .unwrap();
    if matches!(scalar, hir::Ty::Bool | hir::Ty::Unit) {
        let predicate = if scalar == hir::Ty::Bool { "ule" } else { "eq" };
        let bound = usize::from(scalar == hir::Ty::Bool);
        writeln!(out, "  %{name}_canonical = icmp {predicate} i8 %{name}_active, {bound}\n  br i1 %{name}_canonical, label %{name}_payload_ok, label %{name}_payload_error\n{name}_payload_error:").unwrap();
        emit_failure(out, failure.0, FailureKind::EnumPayload, failure.1);
        writeln!(out, "{name}_payload_ok:").unwrap();
    }
    if scalar == hir::Ty::Bool {
        writeln!(out, "  %{name}_value = trunc i8 %{name}_active to i1").unwrap();
        Some((scalar, format!("%{name}_value")))
    } else {
        Some((scalar, format!("%{name}_active")))
    }
}

fn enum_payload_store(
    out: &mut Emission,
    name: &str,
    destination: &str,
    scalar: hir::Ty,
    value: &str,
    offset: usize,
) {
    let pointer = field_pointer(out, &format!("{name}_active_out"), destination, offset);
    if scalar == hir::Ty::Bool {
        writeln!(out, "  %{name}_byte = zext i1 {value} to i8\n  store i8 %{name}_byte, ptr {pointer}, align 1").unwrap();
    } else {
        writeln!(
            out,
            "  store {} {value}, ptr {pointer}, align 1",
            ty(scalar)
        )
        .unwrap();
    }
}

/// A switch validates the tag before reaching any case's active payload read.
/// No destination byte is touched until all runtime checks for that case pass.
/// With no destination this is explicit available-value discard validation.
#[derive(Clone, Copy)]
enum TransferIntent<'a> {
    Move(&'a str),
    Validate,
    Release,
}
impl<'a> TransferIntent<'a> {
    fn destination(self) -> Option<&'a str> {
        match self {
            Self::Move(destination) => Some(destination),
            _ => None,
        }
    }
    fn child<'b>(self, destination: Option<&'b str>) -> TransferIntent<'b> {
        match self {
            Self::Move(_) => TransferIntent::Move(destination.expect("move destination")),
            Self::Validate => TransferIntent::Validate,
            Self::Release => TransferIntent::Release,
        }
    }
}

fn enum_transfer(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    enumeration: EnumId,
    source: &str,
    destination: Option<&str>,
    failure: (&Diagnostics, Span),
) {
    enum_transfer_with_intent(
        out,
        plan,
        name,
        enumeration,
        source,
        destination.map_or(TransferIntent::Release, TransferIntent::Move),
        failure,
    );
}

fn enum_transfer_with_intent(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    enumeration: EnumId,
    source: &str,
    intent: TransferIntent<'_>,
    failure: (&Diagnostics, Span),
) {
    let destination = intent.destination();
    if out.exceeded {
        return;
    }
    let variants = plan
        .witness()
        .declarations()
        .enums()
        .variants(enumeration)
        .expect("verified enum");
    if variants
        .iter()
        .any(|variant| matches!(variant.value_payload(), Some(ValueTy::Owned(_))))
    {
        owned_enum_transfer(out, plan, name, enumeration, source, intent, failure);
        return;
    }
    out.ordinary_visits += 1;
    writeln!(out, "  %{name}_tag = load i32, ptr {source}, align 1\n  switch i32 %{name}_tag, label %{name}_tag_error [").unwrap();
    for variant in variants {
        if !out.expand(Expansion::Transfer) {
            return;
        }
        writeln!(
            out,
            "    i32 {}, label %{name}_variant{}",
            variant.tag(),
            variant.tag()
        )
        .unwrap();
    }
    writeln!(out, "  ]\n{name}_tag_error:").unwrap();
    emit_failure(out, failure.0, FailureKind::EnumTag, failure.1);
    for variant in variants {
        if !out.expand(Expansion::Transfer) {
            return;
        }
        let case = format!("{name}_variant{}", variant.tag());
        writeln!(out, "{case}:").unwrap();
        let active = enum_payload_load(out, plan, &case, source, variant.id(), failure);
        if let Some(destination) = destination {
            writeln!(
                out,
                "  store i32 {}, ptr {destination}, align 1",
                variant.tag()
            )
            .unwrap();
            if let Some((scalar, value)) = active {
                enum_payload_store(
                    out,
                    &case,
                    destination,
                    scalar,
                    &value,
                    variant.payload_offset().expect("active scalar offset"),
                );
            }
        }
        writeln!(out, "  br label %{name}_enum_ok").unwrap();
    }
    writeln!(out, "{name}_enum_ok:").unwrap();
}

/// Each frame is one checked enum declaration on the current containment
/// path. Names use a flat ordinal, so neither retained names nor host recursion
/// grow with a nested variant path. The depth comes from checked layouts.
#[derive(Clone, Copy)]
struct EnumTransferFrame {
    enumeration: EnumId,
    relative: usize,
    ordinal: usize,
    next: usize,
}
// The traversal retains numeric frames, one static leaf iterator, and only a
// fixed number of short generated names. Six printed usize IDs plus suffixes
// bound every name, including nested ordinal/variant/leaf names. This is a
// containment check inside the existing envelope, with no new allocation cap.
#[allow(dead_code)]
struct EnumTransferScratch {
    frames: [Option<EnumTransferFrame>; MAX_CONTAINMENT_DEPTH + 1],
    leaves: ScalarLeaves<'static>,
    names: [String; 32],
    formatting: [std::fmt::Arguments<'static>; 16],
}
const ENUM_TRANSFER_NAME_BYTES: usize = 6 * usize::BITS as usize + 64;
const _: () = assert!(
    size_of::<EnumTransferScratch>()
        + 32 * ENUM_TRANSFER_NAME_BYTES
        + plan::native_storage::FIXED_CARRIER_ALLOWANCE
        < EMITTER_TRANSIENT_BYTES
);
fn enum_node_name(name: &str, ordinal: usize) -> String {
    if ordinal == 0 {
        name.to_owned()
    } else {
        format!("{name}_nested{ordinal}")
    }
}
fn enum_node_pointer(name: &str, frame: EnumTransferFrame, root: &str, output: bool) -> String {
    if frame.ordinal == 0 {
        root.to_owned()
    } else {
        let role = if output { "destination" } else { "source" };
        format!("%{}_{}_ptr", enum_node_name(name, frame.ordinal), role)
    }
}
fn enum_node_switch(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    frame: EnumTransferFrame,
    source: &str,
    destination: Option<&str>,
    failure: (&Diagnostics, Span),
) {
    let node = enum_node_name(name, frame.ordinal);
    if frame.ordinal != 0 {
        field_pointer(out, &format!("{node}_source"), source, frame.relative);
        if let Some(destination) = destination {
            field_pointer(
                out,
                &format!("{node}_destination"),
                destination,
                frame.relative,
            );
        }
    }
    let input = enum_node_pointer(name, frame, source, false);
    out.ordinary_visits += 1;
    writeln!(out, "  %{node}_tag = load i32, ptr {input}, align 1\n  switch i32 %{node}_tag, label %{node}_tag_error [").unwrap();
    for variant in plan
        .witness()
        .declarations()
        .enums()
        .variants(frame.enumeration)
        .expect("verified enum")
    {
        if !out.expand(Expansion::Transfer) {
            return;
        }
        writeln!(
            out,
            "    i32 {}, label %{node}_variant{}",
            variant.tag(),
            variant.tag()
        )
        .unwrap();
    }
    writeln!(out, "  ]\n{node}_tag_error:").unwrap();
    emit_failure(out, failure.0, FailureKind::EnumTag, failure.1);
}

/// Validate all terminal leaves before copying any. Enum chains validate each
/// tag on descent; ancestor tag stores happen only after a complete child has
/// been validated. Thus a bad nested tag or terminal scalar writes no payload.
fn owned_enum_transfer(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    enumeration: EnumId,
    source: &str,
    intent: TransferIntent<'_>,
    failure: (&Diagnostics, Span),
) {
    let destination = intent.destination();
    let first = EnumTransferFrame {
        enumeration,
        relative: 0,
        ordinal: 0,
        next: 0,
    };
    let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
    stack[0] = Some(first);
    let mut length = 1;
    let mut ordinal = 0usize;
    enum_node_switch(out, plan, name, first, source, destination, failure);
    while length != 0 && !out.exceeded {
        let frame = stack[length - 1].expect("checked enum traversal");
        let node = enum_node_name(name, frame.ordinal);
        let variants = plan
            .witness()
            .declarations()
            .enums()
            .variants(frame.enumeration)
            .expect("verified enum");
        let Some(variant) = variants.get(frame.next) else {
            writeln!(out, "{node}_enum_ok:").unwrap();
            length -= 1;
            if length != 0 {
                let parent = stack[length - 1].expect("checked enum parent");
                let parent_name = enum_node_name(name, parent.ordinal);
                if let Some(destination) = destination {
                    let output = enum_node_pointer(name, parent, destination, true);
                    let tag = plan
                        .witness()
                        .declarations()
                        .enums()
                        .variants(parent.enumeration)
                        .expect("verified parent")[parent.next - 1]
                        .tag();
                    writeln!(out, "  store i32 {tag}, ptr {output}, align 1").unwrap();
                }
                writeln!(out, "  br label %{parent_name}_enum_ok").unwrap();
            }
            continue;
        };
        stack[length - 1] = Some(EnumTransferFrame {
            next: frame.next + 1,
            ..frame
        });
        if !out.expand(Expansion::Transfer) {
            return;
        }
        let case = format!("{node}_variant{}", variant.tag());
        writeln!(out, "{case}:").unwrap();
        let input = enum_node_pointer(name, frame, source, false);
        let output = destination.map(|root| enum_node_pointer(name, frame, root, true));
        match variant.value_payload() {
            Some(ValueTy::Owned(AggregateTy::Enum(child))) => {
                ordinal = ordinal.checked_add(1).expect("bounded enum emission");
                let child = EnumTransferFrame {
                    enumeration: child,
                    relative: frame
                        .relative
                        .checked_add(variant.payload_offset().expect("owned payload offset"))
                        .expect("checked payload offset"),
                    ordinal,
                    next: 0,
                };
                stack[length] = Some(child);
                length += 1;
                enum_node_switch(out, plan, name, child, source, destination, failure);
                continue;
            }
            Some(ValueTy::Owned(aggregate)) => {
                let offset = variant.payload_offset().expect("owned payload offset");
                let active = field_pointer(out, &format!("{case}_active"), &input, offset);
                let target = output.as_ref().map(|output| {
                    field_pointer(out, &format!("{case}_active_out"), output, offset)
                });
                active_transfer_with_intent(
                    out,
                    plan,
                    &case,
                    aggregate,
                    &active,
                    intent.child(target.as_deref()),
                    failure,
                );
                if let Some(output) = &output {
                    writeln!(out, "  store i32 {}, ptr {output}, align 1", variant.tag()).unwrap();
                }
            }
            _ => {
                let active = enum_payload_load(out, plan, &case, &input, variant.id(), failure);
                if let Some(output) = &output {
                    writeln!(out, "  store i32 {}, ptr {output}, align 1", variant.tag()).unwrap();
                    if let Some((scalar, value)) = active {
                        enum_payload_store(
                            out,
                            &case,
                            output,
                            scalar,
                            &value,
                            variant.payload_offset().expect("active scalar offset"),
                        );
                    }
                }
            }
        }
        writeln!(out, "  br label %{node}_enum_ok").unwrap();
    }
}

fn active_transfer(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    destination: Option<&str>,
    failure: (&Diagnostics, Span),
) {
    active_transfer_with_intent(
        out,
        plan,
        name,
        aggregate,
        source,
        destination.map_or(TransferIntent::Release, TransferIntent::Move),
        failure,
    );
}

fn active_transfer_with_intent(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    intent: TransferIntent<'_>,
    failure: (&Diagnostics, Span),
) {
    let destination = intent.destination();
    if let AggregateTy::Enum(enumeration) = aggregate {
        enum_transfer_with_intent(out, plan, name, enumeration, source, intent, failure);
        return;
    }
    if !matches!(aggregate, AggregateTy::ByteBuf | AggregateTy::File) {
        for (index, leaf) in plan
            .witness()
            .declarations()
            .leaves(aggregate)
            .expect("verified payload leaves")
            .enumerate()
        {
            if !out.expand(Expansion::Transfer) {
                return;
            }
            let cell = format!("{name}_validate{index}");
            let input = field_pointer(out, &cell, source, leaf.offset);
            let storage = if matches!(leaf.ty, hir::Ty::Bool | hir::Ty::Unit) {
                "i8"
            } else {
                ty(leaf.ty)
            };
            writeln!(out, "  %{cell} = load {storage}, ptr {input}, align 1").unwrap();
            if matches!(leaf.ty, hir::Ty::Bool | hir::Ty::Unit) {
                let predicate = if leaf.ty == hir::Ty::Bool {
                    "ule"
                } else {
                    "eq"
                };
                let bound = usize::from(leaf.ty == hir::Ty::Bool);
                writeln!(out, "  %{cell}_canonical = icmp {predicate} i8 %{cell}, {bound}\n  br i1 %{cell}_canonical, label %{cell}_ok, label %{cell}_error\n{cell}_error:").unwrap();
                emit_failure(out, failure.0, FailureKind::EnumPayload, failure.1);
                writeln!(out, "{cell}_ok:").unwrap();
            }
        }
    }
    resources::leaves(
        out,
        plan,
        &format!("{name}_admit"),
        aggregate,
        source,
        match intent {
            TransferIntent::Release => resources::Action::Release,
            _ => resources::Action::Validate,
        },
        Expansion::Transfer,
        failure,
    );
    if let Some(destination) = destination {
        transfer_leaves(
            out,
            plan,
            &format!("{name}_copy"),
            aggregate,
            (source, destination),
            Expansion::Transfer,
            failure,
        );
    }
}

fn emit_dispatch(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    f: &RawOwnedFunction,
    name: &str,
    match_id: MatchId,
    arm: usize,
    diagnostics: &Diagnostics,
) {
    let descriptor = &f.matches[match_id.0];
    let selected = descriptor.arms[arm];
    let AggregateTy::Enum(enumeration) = f.owners[descriptor.source.0].aggregate() else {
        unreachable!("verified enum match source")
    };
    let declarations = plan.witness().declarations().enums();
    let variant = declarations
        .variant(enumeration, selected.variant)
        .expect("verified match variant");
    let count = declarations
        .variants(enumeration)
        .expect("verified enum")
        .len();
    writeln!(out, "  %{name}_tag = load i32, ptr %o{}, align 1\n  %{name}_valid = icmp ult i32 %{name}_tag, {count}\n  br i1 %{name}_valid, label %{name}_tag_valid, label %{name}_tag_error\n{name}_tag_error:", descriptor.source.0).unwrap();
    emit_failure(out, diagnostics, FailureKind::EnumTag, descriptor.span);
    writeln!(
        out,
        "{name}_tag_valid:\n  %{name}_selected = icmp eq i32 %{name}_tag, {}",
        variant.tag()
    )
    .unwrap();
    if let Some(next) = descriptor.arms.get(arm + 1) {
        writeln!(
            out,
            "  br i1 %{name}_selected, label %b{}, label %b{}",
            selected.entry.0, next.dispatch.0
        )
        .unwrap();
    } else {
        // The final dispatch is independently checked, including singleton enums.
        writeln!(
            out,
            "  br i1 %{name}_selected, label %b{}, label %{name}_tag_error",
            selected.entry.0
        )
        .unwrap();
    }
}

fn transfer(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    pointers: (&str, &str),
    failure: (&Diagnostics, Span),
) {
    let (source, destination) = pointers;
    if out.exceeded {
        return;
    }
    out.ordinary_visits += 1;
    let record = match aggregate {
        AggregateTy::Record(record) => record,
        AggregateTy::Enum(_) => unreachable!("enum transfers require checked tag-directed path"),
        AggregateTy::ByteBuf | AggregateTy::File => {
            resources::leaves(
                out,
                plan,
                name,
                aggregate,
                source,
                resources::Action::Move(destination),
                Expansion::Transfer,
                failure,
            );
            return;
        }
        AggregateTy::FixedArray(array) => {
            if array.length() == 0 {
                if !out.expand(Expansion::Transfer) {
                    return;
                }
                let scalar = sentinel_ty(array);
                writeln!(out, "  %{name}_empty = load {scalar}, ptr {source}, align 1\n  store {scalar} %{name}_empty, ptr {destination}, align 1").unwrap();
            }
            for i in 0..array.length() {
                if !out.expand(Expansion::Transfer) {
                    return;
                }
                let offset = i
                    .checked_mul(array.stride())
                    .expect("verified array offset");
                let input = field_pointer(out, &format!("{name}_in{i}"), source, offset);
                let output = field_pointer(out, &format!("{name}_out{i}"), destination, offset);
                let scalar = ty(array.element());
                writeln!(out, "  %{name}_element{i} = load {scalar}, ptr {input}, align 1\n  store {scalar} %{name}_element{i}, ptr {output}, align 1").unwrap();
            }
            return;
        }
    };
    let fields = plan
        .witness()
        .declarations()
        .fields(record)
        .expect("verified record");
    if fields.is_empty() {
        if !out.expand(Expansion::Transfer) {
            return;
        }
        writeln!(out, "  %{name}_empty = load i8, ptr {source}, align 1\n  store i8 %{name}_empty, ptr {destination}, align 1").unwrap();
        return;
    }
    // Scalar-only records retain their field order, instruction text and
    // expansion counts. Composite records use the same bounded lazy walk.
    transfer_leaves(
        out,
        plan,
        name,
        aggregate,
        (source, destination),
        Expansion::Transfer,
        failure,
    );
}

/// The declaration iterator stores at most one frame per admitted containment
/// level. It yields initialized scalar/sentinel cells, never record padding.
#[allow(clippy::too_many_arguments)]
fn transfer_leaves(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    pointers: (&str, &str),
    kind: Expansion,
    failure: (&Diagnostics, Span),
) {
    let (source, destination) = pointers;
    resources::leaves(
        out,
        plan,
        &format!("{name}_owned"),
        aggregate,
        source,
        resources::Action::Move(destination),
        kind,
        failure,
    );
    if matches!(aggregate, AggregateTy::ByteBuf | AggregateTy::File) {
        return;
    }
    let leaves = plan
        .witness()
        .declarations()
        .leaves(aggregate)
        .expect("verified aggregate leaves");
    for (i, leaf) in leaves.enumerate() {
        if !out.expand(kind) {
            return;
        }
        #[cfg(test)]
        {
            out.field_visits += 1;
        }
        let input = field_pointer(out, &format!("{name}_in{i}"), source, leaf.offset);
        let output = field_pointer(out, &format!("{name}_out{i}"), destination, leaf.offset);
        writeln!(out, "  %{name}_field{i} = load {}, ptr {input}, align 1\n  store {} %{name}_field{i}, ptr {output}, align 1", ty(leaf.ty), ty(leaf.ty)).unwrap();
    }
}

// Independently checked compiler-work facts; no native allocation, witness,
// or emitter capability escapes. The caller discards them after the guards.
pub(super) fn admit_inventory_policy(
    execution: &ExecutionPlan<'_>,
    limits: (usize, usize),
) -> Result<plan::native_storage::NativeInventories, Box<Diagnostic>> {
    let counts = plan::native_storage::NativeInventories::checked(execution)
        .map_err(|failure| reject(failure.name, failure.span))?;
    // An empty library has zero inventory but is not an executable entry.
    // Existing entry validation remains a separate, earlier native gate.
    if let Some(function) = execution.witness().functions().first() {
        limit(
            counts.items(),
            limits.0.min(MAX_NATIVE_INVENTORY_ITEMS),
            "aggregate compiler inventory items",
            function.span,
        )?;
        limit(
            counts.owner_width(),
            limits.1.min(MAX_NATIVE_OWNER_WIDTH),
            "aggregate owner width cells",
            function.span,
        )?;
    }
    Ok(counts)
}

fn emit(
    storage: &NativeStoragePlan<'_, '_>,
    entry: hir::DefId,
    diagnostics: &Diagnostics,
    fuel: usize,
    out: &mut Emission,
) {
    let plan = storage.execution();
    out.resources = plan.uses_dynamic_resources();
    let guarded = storage.guarded();
    out.write_str("; Oxid private owned native ABI 1\nsource_filename = \"oxid-owned-native\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_unit()\ndeclare void @__oxid_overflow(ptr, i64) noreturn\ndeclare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)\n").unwrap();
    if out.policy.recursive() {
        hosted_frames::globals(out);
    }
    if plan.uses_wide_scalars() {
        out.write_str("declare i32 @__oxid_print_i64(i64)\ndeclare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)\ndeclare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)\ndeclare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)\n").unwrap();
    }
    if matches!(
        plan.witness().functions()[entry.0].result,
        ValueTy::Scalar(hir::Ty::U64 | hir::Ty::Usize)
    ) {
        out.write_str("declare i32 @__oxid_print_u64(i64)\n")
            .unwrap();
    }
    if plan.uses_integer_extension() {
        for width in [8, 16] {
            for operation in ["sadd", "ssub", "smul"] {
                writeln!(out, "declare {{ i{width}, i1 }} @llvm.{operation}.with.overflow.i{width}(i{width}, i{width})").unwrap();
            }
        }
        for width in [8, 16, 32, 64] {
            for operation in ["uadd", "usub", "umul"] {
                writeln!(out, "declare {{ i{width}, i1 }} @llvm.{operation}.with.overflow.i{width}(i{width}, i{width})").unwrap();
            }
        }
    }
    if plan.witness().functions()[entry.0].result == ValueTy::Scalar(hir::Ty::Char) {
        out.write_str("declare i32 @__oxid_print_char(i32)\n")
            .unwrap();
    }
    if plan.uses_floats() {
        out.write_str("declare void @__oxid_float_runtime()\n")
            .unwrap();
    }
    if matches!(plan.witness().functions()[entry.0].result, ValueTy::Scalar(ty) if ty.floating()) {
        out.write_str("declare i32 @__oxid_print_float_bits(i64, i32)\n")
            .unwrap();
    }
    if plan.uses_quad_scalars() {
        out.write_str("declare i32 @__oxid_print_i128_parts(i64, i64)\ndeclare i32 @__oxid_print_u128_parts(i64, i64)\ndeclare i32 @__oxid_checked_mul128_parts(i64, i64, i64, i64, i32, ptr, ptr)\ndeclare i32 @__oxid_udivmod128_parts(i64, i64, i64, i64, ptr, ptr, ptr, ptr)\n").unwrap();
        for operation in ["sadd", "ssub", "uadd", "usub"] {
            writeln!(
                out,
                "declare {{ i128, i1 }} @llvm.{operation}.with.overflow.i128(i128, i128)"
            )
            .unwrap();
        }
    }
    if out.resources {
        out.write_str("declare void @__oxid_bytebuf_start()\ndeclare void @__oxid_bytebuf_cleanup()\ndeclare i32 @__oxid_bytebuf_finish()\ndeclare i32 @__oxid_bytebuf_valid(ptr)\ndeclare i32 @__oxid_file_valid(ptr)\ndeclare void @__oxid_file_drop(ptr)\ndeclare i32 @__oxid_file_delivery(i32, ptr, ptr, i32, ptr, ptr, i64)\ndeclare void @__oxid_bytebuf_drop(ptr)\ndeclare i32 @__oxid_bytebuf_operation(i32, ptr, i32, i32, ptr, i32, ptr, ptr)\ndeclare i32 @__oxid_bytebuf_view(ptr, i32, i32, i32, ptr, ptr)\ndeclare i32 @__oxid_binary_delivery(i32, ptr, i32, ptr, ptr, i64)\ndeclare void @llvm.memset.p0.i64(ptr, i8, i64, i1 immarg)\n").unwrap();
        if out.policy.hosted() {
            out.write_str("declare i32 @__oxid_file_delivery_hosted(i32, ptr, ptr, i32, ptr, ptr, i64)\ndeclare i32 @__oxid_bytebuf_operation_hosted(i32, ptr, i32, i32, ptr, i32, ptr, ptr)\ndeclare i32 @__oxid_binary_delivery_hosted(i32, ptr, i32, ptr, ptr, i64)\n").unwrap();
        }
    }
    if plan.witness().builtin_function().is_some() {
        out.write_str("declare i32 @__oxid_read_stdin_byte(ptr)\n")
            .unwrap();
    }
    if plan.has_program_arguments() {
        out.write_str("declare void @__oxid_arguments_start(i32, ptr)\ndeclare i32 @__oxid_arg_count()\ndeclare i32 @__oxid_arg_bytes(i32, ptr, i32, ptr, ptr, i64)\n").unwrap();
        if out.policy.hosted() {
            out.write_str("declare i32 @__oxid_arg_bytes_hosted(i32, ptr, i32, ptr, ptr, i64)\n")
                .unwrap();
        }
    }
    if plan.witness().builtin_output_function().is_some() {
        out.write_str("declare i32 @__oxid_write_stdout_byte(ptr)\n")
            .unwrap();
    }
    if out.policy.process() || plan.has_file_delivery() {
        out.write_str("declare i32 @__oxid_process_setup()\ndeclare void @__oxid_process_failure(ptr, i64) noreturn\n")
            .unwrap();
    }
    for (id, message) in diagnostics.messages.iter().enumerate() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        write!(
            out,
            "@__oxid_owned_error_{id} = private unnamed_addr constant [{} x i8] c\"",
            message.len()
        )
        .unwrap();
        for byte in message.bytes() {
            if !out.expand(Expansion::Diagnostic) {
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
        out.ordinary_visits += 1;
        emit_function(storage, f.id, diagnostics, out);
    }
    if out.exceeded {
        return;
    }
    let root = &plan.witness().functions()[entry.0];
    let ValueTy::Scalar(result) = root.result else {
        unreachable!("entry gate")
    };
    if plan.has_program_arguments() {
        out.write_str("\ndefine i32 @main(i32 %argc, ptr %argv) {\nentry:\n  call void @__oxid_arguments_start(i32 %argc, ptr %argv)\n").unwrap();
    } else {
        out.write_str("\ndefine i32 @main() {\nentry:\n").unwrap();
    }
    if out.policy.process() || plan.has_file_delivery() {
        // Signal safety precedes even the root activation/fuel diagnostic.
        out.write_str("  %setup = call i32 @__oxid_process_setup()\n  %setup_ok = icmp eq i32 %setup, 0\n  br i1 %setup_ok, label %process_ready, label %setup_error\nsetup_error:\n  ret i32 74\nprocess_ready:\n").unwrap();
    }
    if out.resources {
        out.write_str("  call void @__oxid_bytebuf_start()\n")
            .unwrap();
    }
    if storage.wrapper_fuel_bytes() != 0 {
        writeln!(
            out,
            "  %fuel = alloca i64, align 8\n  store i64 {fuel}, ptr %fuel, align 8"
        )
        .unwrap();
        emit_guard(
            out,
            diagnostics,
            "root",
            1 + plan.function(entry).usage().activation_fuel_cells(),
            root.span,
        );
    }
    let usage = frame_usage(plan, entry);
    if out.policy.recursive() {
        let initial_bytes =
            runtime_storage_bytes(plan, guarded, out.policy).expect("admitted runtime storage");
        hosted_frames::reset(out, initial_bytes);
        emit_frame_enter(out, diagnostics, "hosted_root", usage, root.span);
    }
    writeln!(
        out,
        "  %value = call {} @__oxid_owned_fn_{}({})",
        ty(result),
        entry.0,
        if guarded { "ptr %fuel" } else { "" }
    )
    .unwrap();
    if out.policy.recursive() {
        hosted_frames::leave(out, "hosted_root", usage);
    }
    if out.resources {
        out.write_str("  %resource_status = call i32 @__oxid_bytebuf_finish()\n  %resources_released = icmp eq i32 %resource_status, 0\n  br i1 %resources_released, label %resources_complete, label %resources_error\nresources_error:\n").unwrap();
        emit_failure(out, diagnostics, FailureKind::ResourceDescriptor, root.span);
        out.write_str("resources_complete:\n").unwrap();
    }
    if out.policy.process() {
        // Unsigned comparison rejects negatives without truncating i32 status.
        out.write_str("  %status_valid = icmp ule i32 %value, 255\n  br i1 %status_valid, label %process_complete, label %process_status_error\nprocess_status_error:\n").unwrap();
        emit_failure(out, diagnostics, FailureKind::ProcessStatus, root.span);
        out.write_str("process_complete:\n  ret i32 %value\n}\n")
            .unwrap();
        return;
    }
    match result {
        hir::Ty::Char => out.write_str("  %status = call i32 @__oxid_print_char(i32 %value)\n").unwrap(),
        hir::Ty::F32 => out.write_str("  %float_bits = bitcast float %value to i32\n  %output_value = zext i32 %float_bits to i64\n  %status = call i32 @__oxid_print_float_bits(i64 %output_value, i32 32)\n").unwrap(),
        hir::Ty::F64 => out.write_str("  %output_value = bitcast double %value to i64\n  %status = call i32 @__oxid_print_float_bits(i64 %output_value, i32 64)\n").unwrap(),

        hir::Ty::Bool => out.write_str("  %wide = zext i1 %value to i32\n  %status = call i32 @__oxid_print_bool(i32 %wide)\n").unwrap(),
        hir::Ty::I32 => out.write_str("  %status = call i32 @__oxid_print_i32(i32 %value)\n").unwrap(),
        hir::Ty::I64 => out.write_str("  %status = call i32 @__oxid_print_i64(i64 %value)\n").unwrap(),
        hir::Ty::U8 => out.write_str("  %output_value = zext i8 %value to i32\n  %status = call i32 @__oxid_print_i32(i32 %output_value)\n").unwrap(),
        hir::Ty::I8 | hir::Ty::I16 => {
            writeln!(out, "  %output_value = sext {} %value to i64\n  %status = call i32 @__oxid_print_i64(i64 %output_value)", ty(result)).unwrap();
        }
        hir::Ty::U16 | hir::Ty::U32 => {
            writeln!(out, "  %output_value = zext {} %value to i64\n  %status = call i32 @__oxid_print_i64(i64 %output_value)", ty(result)).unwrap();
        }
        hir::Ty::Isize => out.write_str("  %status = call i32 @__oxid_print_i64(i64 %value)\n").unwrap(),
        hir::Ty::I128 | hir::Ty::U128 => {
            let function = if result.signed_integer() { "i128" } else { "u128" };
            writeln!(out, "  %output_low = trunc i128 %value to i64\n  %output_shifted = lshr i128 %value, 64\n  %output_high = trunc i128 %output_shifted to i64\n  %status = call i32 @__oxid_print_{function}_parts(i64 %output_low, i64 %output_high)").unwrap();
        }
        hir::Ty::U64 | hir::Ty::Usize => out.write_str("  %status = call i32 @__oxid_print_u64(i64 %value)\n").unwrap(),
        hir::Ty::Unit => out.write_str("  %status = call i32 @__oxid_print_unit()\n").unwrap(),
    }
    out.write_str("  ret i32 %status\n}\n").unwrap();
}

#[derive(Clone, Copy)]
enum Continuation {
    Unsplit,
    Aggregate,
    Arithmetic,
    Conversion,
    Bounds,
    Enum,
    Input,
    Output,
}
impl Continuation {
    fn suffix(self) -> Option<&'static str> {
        match self {
            Self::Unsplit => None,
            Self::Aggregate => Some("aggregate_ok"),
            Self::Arithmetic => Some("checked_ok"),
            Self::Conversion => Some("conversion_ok"),
            Self::Bounds => Some("bounds_ok"),
            Self::Enum => Some("enum_ok"),
            Self::Input => Some("input_ok"),
            Self::Output => Some("output_ok"),
        }
    }
}
/// Exhaustive classification shared by operation emission and phi predecessors.
fn continuation(f: &RawOwnedFunction, instruction: &OwnedInstruction) -> Continuation {
    match instruction {
        OwnedInstruction::DestructureRecord { .. } => Continuation::Aggregate,
        OwnedInstruction::PrepareBorrowRange { .. } => Continuation::Bounds,
        OwnedInstruction::Experimental { .. } => Continuation::Unsplit,
        OwnedInstruction::ReadStdin { .. } => Continuation::Input,
        OwnedInstruction::WriteStdout { .. } => Continuation::Output,
        OwnedInstruction::ConsumeVariant { .. }
        | OwnedInstruction::ConsumeOwnedVariant { .. }
        | OwnedInstruction::ConstructOwnedEnum { .. } => Continuation::Enum,
        OwnedInstruction::MoveInitialize { source, .. }
        | OwnedInstruction::Replace { source, .. }
        | OwnedInstruction::PrepareOwned { source, .. }
        | OwnedInstruction::Discard(source) => {
            if matches!(f.owners[source.0].aggregate(), AggregateTy::Enum(_)) {
                Continuation::Enum
            } else {
                Continuation::Unsplit
            }
        }
        OwnedInstruction::Scalar(statement) => match statement {
            Statement::Assign(assign) => match assign.value {
                Rvalue::CharacterConvert {
                    op: hir::ConversionOp::ToCharChecked,
                    ..
                } => Continuation::Conversion,
                Rvalue::CharacterConvert { .. } => Continuation::Unsplit,
                Rvalue::Bitwise { op, .. } if op.shift() => Continuation::Arithmetic,
                Rvalue::Bitwise { .. } | Rvalue::BitNot { .. } => Continuation::Unsplit,
                Rvalue::CheckedI32 { .. }
                | Rvalue::CheckedNegateI32 { .. }
                | Rvalue::CheckedI64 { .. }
                | Rvalue::CheckedNegateI64 { .. } => Continuation::Arithmetic,
                Rvalue::CheckedInteger { .. } | Rvalue::CheckedNegateInteger { .. } => {
                    Continuation::Arithmetic
                }
                Rvalue::CheckedI32ToU8 { .. }
                | Rvalue::CheckedI64ToI32 { .. }
                | Rvalue::CheckedI64ToU8 { .. }
                | Rvalue::CheckedIntegerCast { .. } => Continuation::Conversion,
                Rvalue::Load(_)
                | Rvalue::U8ToI32 { .. }
                | Rvalue::WidenI64 { .. }
                | Rvalue::NotBool { .. }
                | Rvalue::Bool(_)
                | Rvalue::I32(_)
                | Rvalue::I64(_)
                | Rvalue::Integer { .. }
                | Rvalue::FloatConvert { .. }
                | Rvalue::Character(_)
                | Rvalue::Float { .. }
                | Rvalue::FloatArithmetic { .. }
                | Rvalue::FloatNegate { .. }
                | Rvalue::Integer128 { .. }
                | Rvalue::U8(_)
                | Rvalue::Unit
                | Rvalue::Copy(_)
                | Rvalue::CompareScalar { .. } => Continuation::Unsplit,
            },
            Statement::Initialize { .. } | Statement::Store { .. } => Continuation::Unsplit,
        },
        OwnedInstruction::ReadIndex { .. }
        | OwnedInstruction::WriteIndex { .. }
        | OwnedInstruction::ReadProjection { index: Some(_), .. }
        | OwnedInstruction::WriteProjection { index: Some(_), .. } => Continuation::Bounds,
        OwnedInstruction::StorageLive(_)
        | OwnedInstruction::StorageEnd(_)
        | OwnedInstruction::Construct { .. }
        | OwnedInstruction::ConstructEnum { .. }
        | OwnedInstruction::ConstructArray { .. }
        | OwnedInstruction::ConstructComposite { .. }
        | OwnedInstruction::ReadProjection { index: None, .. }
        | OwnedInstruction::WriteProjection { index: None, .. }
        | OwnedInstruction::ProjectionLength { .. }
        | OwnedInstruction::ReadField { .. }
        | OwnedInstruction::WriteField { .. }
        | OwnedInstruction::ArrayLength { .. }
        | OwnedInstruction::OpenCall(_)
        | OwnedInstruction::PrepareScalar { .. }
        | OwnedInstruction::PrepareBorrow { .. } => Continuation::Unsplit,
    }
}
fn exit_label(f: &RawOwnedFunction, block: usize, guarded: bool, out: &mut Emission) -> String {
    if guarded {
        return format!(
            "f{}_b{block}_g{}_ok",
            f.id.0,
            f.blocks[block].statements.len() + 1
        );
    }
    for (i, statement) in f.blocks[block].statements.iter().enumerate().rev() {
        out.predecessor_visits += 1;
        if let Some(suffix) = continuation(f, &statement.kind).suffix() {
            return format!("f{}_b{block}_i{i}_{suffix}", f.id.0);
        }
    }
    format!("b{block}")
}
fn emit_function(
    module_storage: &NativeStoragePlan<'_, '_>,
    id: hir::DefId,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    if out.exceeded {
        return;
    }
    out.ordinary_visits += 1;
    let plan = module_storage.execution();
    let guarded = module_storage.guarded();
    let storage = module_storage.function(id);
    let f = storage.raw();
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
        out.ordinary_visits += 1;
        let t = match parameter {
            ParameterBinding::Scalar(l) => ty(f.locals[l.0].ty),
            _ => "ptr",
        };
        write!(out, "{separator}{t} %arg{i}").unwrap();
        if let ParameterBinding::Reference(reference) = parameter {
            if matches!(
                f.references[reference.0].referent(),
                BorrowedTy::ScalarSlice(_)
            ) {
                write!(out, ", i32 %arg{i}_length").unwrap();
            }
        }
        separator = ", ";
    }
    out.write_str(") noinline {\nentry:\n").unwrap();
    if storage.scalar_width() == 16 {
        out.write_str("  %integer128_scratch = alloca [4 x i64], align 8\n")
            .unwrap();
        for word in 0..4 {
            writeln!(
                out,
                "  %integer128_word{word} = getelementptr i8, ptr %integer128_scratch, i64 {}",
                word * 8
            )
            .unwrap();
        }
    }
    let slots = storage.scalar_slots();
    if slots != 0 {
        let scalar_width = storage.scalar_width();
        let scalar_bits = scalar_width * 8;
        writeln!(
            out,
            "  %scalars = alloca [{slots} x i{scalar_bits}], align {scalar_width}"
        )
        .unwrap();
    }
    // Round the byte alloca itself to the four-byte size charged in Dnative.
    if storage.owner_bytes() != 0 {
        writeln!(
            out,
            "  %owners = alloca [{} x i8], align {}",
            storage.owner_bytes(),
            storage.owner_alignment()
        )
        .unwrap();
    }
    if let Some(scratch) = storage.input_scratch() {
        debug_assert_eq!(scratch.len(), 1024);
        // This suffix is part of the already admitted owner allocation. It is
        // allocated once at entry and reused by every attempt in this frame.
        writeln!(
            out,
            "  %input_scratch = getelementptr i8, ptr %owners, i64 {}",
            scratch.start
        )
        .unwrap();
    }
    if let Some(scratch) = storage.output_scratch() {
        debug_assert_eq!(scratch.len(), 1024);
        // A distinct builtin activation owns output staging. It cannot alias
        // the source view or input scratch, and is admitted before any effect.
        writeln!(
            out,
            "  %output_scratch = getelementptr i8, ptr %owners, i64 {}",
            scratch.start
        )
        .unwrap();
    }
    if storage.reference_slots() != 0 {
        writeln!(
            out,
            "  %references = alloca [{} x ptr], align 8",
            storage.reference_slots()
        )
        .unwrap();
    }
    let slice_slots = storage.slice_slots();
    if slice_slots != 0 {
        writeln!(
            out,
            "  %slice_lengths = alloca [{slice_slots} x i32], align 4"
        )
        .unwrap();
        // Only slice views retain lengths. Exact references keep their single
        // pointer layout and array owners retain their existing payload layout.
        for mapping in storage.slice_mappings() {
            if out.exceeded {
                return;
            }
            out.ordinary_visits += 1;
            if let Some(offset) = mapping.offset {
                let prefix = if mapping.loan { "ll" } else { "rl" };
                writeln!(
                    out,
                    "  %{prefix}{} = getelementptr i8, ptr %slice_lengths, i64 {offset}",
                    mapping.logical
                )
                .unwrap();
            }
        }
    }
    for i in 0..slots {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        writeln!(
            out,
            "  %s{i} = getelementptr i8, ptr %scalars, i64 {}",
            storage.scalar_offset(i)
        )
        .unwrap();
    }
    for i in 0..f.owners.len() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        writeln!(
            out,
            "  %o{i} = getelementptr i8, ptr %owners, i64 {}",
            storage.owner_offset(OwnerPlaceId(i))
        )
        .unwrap();
        resources::initialize(out, plan, f.owners[i].aggregate(), &format!("%o{i}"));
    }
    for i in 0..storage.reference_slots() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        writeln!(
            out,
            "  %r{i} = getelementptr i8, ptr %references, i64 {}",
            storage.reference_offset(i)
        )
        .unwrap();
    }
    for (i, parameter) in f.parameters.iter().enumerate() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
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
                writeln!(out, "  store ptr %arg{i}, ptr %r{}, align 8", r.0).unwrap();
                if matches!(f.references[r.0].referent(), BorrowedTy::ScalarSlice(_)) {
                    writeln!(out, "  store i32 %arg{i}_length, ptr %rl{}, align 4", r.0).unwrap();
                }
            }
            ParameterBinding::Owned(o) => transfer_checked(
                out,
                plan,
                &name,
                f.owners[o.0].aggregate(),
                (&format!("%arg{i}"), &format!("%o{}", o.0)),
                (diagnostics, f.span),
            ),
        }
    }
    writeln!(out, "  br label %b{}", f.entry.0).unwrap();
    for (b, block) in f.blocks.iter().enumerate() {
        if out.exceeded {
            return;
        }
        out.ordinary_visits += 1;
        writeln!(out, "b{b}:").unwrap();
        if let Some(m) = &block.merge {
            let name = format!("f{}_b{b}_merge", id.0);
            let [left, right] = m.incoming;
            // Select storage, then read exactly the taken input after charging.
            // A value phi would read an uninitialized slot on the untaken arm.
            let left_label = exit_label(f, left.predecessor.0, guarded, out);
            let right_label = exit_label(f, right.predecessor.0, guarded, out);
            writeln!(
                out,
                "  %{name}_slot = phi ptr [ %s{}, %{} ], [ %s{}, %{} ]",
                left.value.local.0, left_label, right.value.local.0, right_label
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
            out.ordinary_visits += 1;
            let name = format!("f{}_b{b}_i{i}", id.0);
            if guarded
                && !matches!(
                    statement.kind,
                    OwnedInstruction::ReadStdin { .. } | OwnedInstruction::WriteStdout { .. }
                )
            {
                emit_guard(
                    out,
                    diagnostics,
                    &format!("f{}_b{b}_g{}", id.0, i + 1),
                    plan.statement_cost(id, &statement.kind),
                    plan::instruction_span(statement),
                );
            }
            emit_statement(&storage, &name, statement, diagnostics, out);
        }
        if out.exceeded {
            return;
        }
        let term = block.terminator.as_ref().expect("verified terminator");
        if guarded
            && plan.binary_delivery_cost(id).is_none()
            && plan.file_delivery_cost(id).is_none()
        {
            emit_guard(
                out,
                diagnostics,
                &format!("f{}_b{b}_g{}", id.0, block.statements.len() + 1),
                plan.terminator_cost(id, &term.kind),
                term.span,
            );
        }
        emit_terminator(
            &storage,
            &format!("f{}_b{b}_term", id.0),
            term,
            (diagnostics, guarded),
            out,
        );
    }
    out.write_str("}\n").unwrap();
}

/// The sealed builtin owns both the result extent and its admitted scratch
/// suffix. Its exclusive slice pointer follows the existing verified ABI;
/// capacity is checked before any input, result store, or destination write.
/// Only successful/EOF paths enter the prepaid, infallible commit loop.
fn emit_read_stdin(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    places: (ReferenceParamId, OwnerPlaceId),
    failure: (&Diagnostics, Span),
    out: &mut Emission,
) {
    let plan = storage.execution();
    let id = storage.id();
    let (buffer, destination) = places;
    let (diagnostics, span) = failure;
    let scratch = storage
        .input_scratch()
        .expect("verified input scratch suffix");
    debug_assert_eq!(plan.witness().builtin_function(), Some(id));
    debug_assert_eq!(scratch.len(), 1024);
    let enumeration = plan
        .witness()
        .builtin_enumeration()
        .expect("verified input enum");
    let variants = plan
        .witness()
        .declarations()
        .enums()
        .variants(enumeration)
        .expect("verified input variants");
    let [eof, full, io_error] = variants else {
        unreachable!("verified input result shape")
    };
    let result = format!("%o{}", destination.0);
    let result_extent = plan
        .witness()
        .declarations()
        .aggregate_layout(AggregateTy::Enum(enumeration))
        .expect("verified input result layout")
        .size();
    debug_assert!(storage.owner_offset(destination) + result_extent <= scratch.start);
    let suffix = continuation(
        &plan.witness().functions()[id.0],
        &OwnedInstruction::ReadStdin {
            buffer,
            destination,
        },
    )
    .suffix()
    .expect("input continuation");
    // Unsigned <= rejects negative i32 lengths as well as lengths above 1024.
    writeln!(out, "  %{name}_capacity = load i32, ptr %rl{}, align 4\n  %{name}_capacity_valid = icmp ule i32 %{name}_capacity, 1024\n  br i1 %{name}_capacity_valid, label %{name}_capacity_ok, label %{name}_capacity_error\n{name}_capacity_error:", buffer.0).unwrap();
    emit_failure(out, diagnostics, FailureKind::InputCapacity, span);
    writeln!(out, "{name}_capacity_ok:\n  %{name}_capacity64 = zext i32 %{name}_capacity to i64\n  %{name}_core_cost = add i64 %{name}_capacity64, 4").unwrap();
    emit_guard_value(
        out,
        diagnostics,
        &format!("{name}_core"),
        format!("%{name}_core_cost"),
        span,
    );
    // All storage is already reserved in this frame. No allocation or fallible
    // destination/result preparation remains after the first adapter attempt.
    writeln!(out, "  %{name}_buffer = load ptr, ptr %r{}, align 8\n  br label %{name}_read_check\n{name}_read_check:\n  %{name}_staged = phi i32 [ 0, %{name}_core_ok ], [ %{name}_next, %{name}_byte ], [ %{name}_staged, %{name}_retry ]\n  %{name}_at_capacity = icmp eq i32 %{name}_staged, %{name}_capacity\n  br i1 %{name}_at_capacity, label %{name}_full, label %{name}_attempt\n{name}_attempt:", buffer.0).unwrap();
    emit_guard(out, diagnostics, &format!("{name}_read"), 1, span);
    // One debit precedes exactly one read, including EOF, EINTR and errors.
    // The adapter writes staging only; bytes are inspected later only when the
    // success path has established the precise initialized prefix.
    writeln!(out, "  %{name}_staged64 = zext i32 %{name}_staged to i64\n  %{name}_byte_ptr = getelementptr i8, ptr %input_scratch, i64 %{name}_staged64\n  %{name}_status = call i32 @__oxid_read_stdin_byte(ptr %{name}_byte_ptr)\n  switch i32 %{name}_status, label %{name}_io_error [\n    i32 1, label %{name}_byte\n    i32 0, label %{name}_eof\n    i32 -1, label %{name}_retry\n  ]\n{name}_byte:\n  %{name}_next = add i32 %{name}_staged, 1\n  br label %{name}_read_check\n{name}_retry:\n  br label %{name}_read_check\n{name}_full:\n  br label %{name}_commit_start\n{name}_eof:\n  br label %{name}_commit_start\n{name}_commit_start:\n  %{name}_is_eof = phi i1 [ false, %{name}_full ], [ true, %{name}_eof ]\n  br label %{name}_commit_check\n{name}_commit_check:\n  %{name}_written = phi i32 [ 0, %{name}_commit_start ], [ %{name}_written_next, %{name}_commit_byte ]\n  %{name}_has_byte = icmp ult i32 %{name}_written, %{name}_staged\n  br i1 %{name}_has_byte, label %{name}_commit_byte, label %{name}_result\n{name}_commit_byte:\n  %{name}_written64 = zext i32 %{name}_written to i64\n  %{name}_source = getelementptr i8, ptr %input_scratch, i64 %{name}_written64\n  %{name}_byte_value = load i8, ptr %{name}_source, align 1\n  %{name}_wide_byte = zext i8 %{name}_byte_value to i32\n  %{name}_destination = getelementptr i32, ptr %{name}_buffer, i64 %{name}_written64\n  store i32 %{name}_wide_byte, ptr %{name}_destination, align 1\n  %{name}_written_next = add i32 %{name}_written, 1\n  br label %{name}_commit_check\n{name}_result:\n  br i1 %{name}_is_eof, label %{name}_result_eof, label %{name}_result_full\n{name}_result_eof:").unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    writeln!(out, "  store i32 {}, ptr {result}, align 1", eof.tag()).unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    enum_payload_store(
        out,
        &format!("{name}_eof_result"),
        &result,
        eof.payload().expect("verified Eof payload"),
        &format!("%{name}_staged"),
        eof.payload_offset().expect("verified Eof offset"),
    );
    writeln!(out, "  br label %{name}_{suffix}\n{name}_result_full:").unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    writeln!(
        out,
        "  store i32 {}, ptr {result}, align 1\n  br label %{name}_{suffix}\n{name}_io_error:",
        full.tag()
    )
    .unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    // Nullary outcomes never read or write the inactive payload/padding.
    writeln!(
        out,
        "  store i32 {}, ptr {result}, align 1\n  br label %{name}_{suffix}\n{name}_{suffix}:",
        io_error.tag()
    )
    .unwrap();
}

/// The canonical shared view is already bound by the verified internal ABI.
/// Capacity and storage are preflighted before reads; the complete validated
/// byte image is staged before any attempt. The only post-effect failure point
/// in the effect itself is the deliberately metered next-attempt fuel guard.
fn emit_write_stdout(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    places: (ReferenceParamId, OwnerPlaceId),
    failure: (&Diagnostics, Span),
    out: &mut Emission,
) {
    let plan = storage.execution();
    let id = storage.id();
    let (buffer, destination) = places;
    let (diagnostics, span) = failure;
    let scratch = storage
        .output_scratch()
        .expect("verified output scratch suffix");
    debug_assert_eq!(plan.witness().builtin_output_function(), Some(id));
    debug_assert_eq!(scratch.len(), 1024);
    debug_assert!(out.policy.process());
    let enumeration = plan
        .witness()
        .builtin_output_enumeration()
        .expect("verified output enum");
    let variants = plan
        .witness()
        .declarations()
        .enums()
        .variants(enumeration)
        .expect("verified output variants");
    let [complete, invalid_input, io_error] = variants else {
        unreachable!("verified output result shape")
    };
    let result = format!("%o{}", destination.0);
    let result_extent = plan
        .witness()
        .declarations()
        .aggregate_layout(AggregateTy::Enum(enumeration))
        .expect("verified output result layout")
        .size();
    debug_assert!(storage.owner_offset(destination) + result_extent <= scratch.start);
    let suffix = continuation(
        &plan.witness().functions()[id.0],
        &OwnedInstruction::WriteStdout {
            buffer,
            destination,
        },
    )
    .suffix()
    .expect("output continuation");
    // Unsigned <= rejects negative i32 lengths and malformed oversized views.
    writeln!(out, "  %{name}_capacity = load i32, ptr %rl{}, align 4\n  %{name}_capacity_valid = icmp ule i32 %{name}_capacity, 1024\n  br i1 %{name}_capacity_valid, label %{name}_capacity_ok, label %{name}_capacity_error\n{name}_capacity_error:", buffer.0).unwrap();
    emit_failure(out, diagnostics, FailureKind::OutputCapacity, span);
    writeln!(out, "{name}_capacity_ok:\n  %{name}_capacity64 = zext i32 %{name}_capacity to i64\n  %{name}_core_cost = add i64 %{name}_capacity64, 4").unwrap();
    emit_guard_value(
        out,
        diagnostics,
        &format!("{name}_core"),
        format!("%{name}_core_cost"),
        span,
    );
    // Validation includes the last cell. Neither invalid input nor an empty
    // view reaches the byte helper, and no source cell is read after staging.
    writeln!(out, "  %{name}_buffer = load ptr, ptr %r{}, align 8\n  br label %{name}_stage_check\n{name}_stage_check:\n  %{name}_validated = phi i32 [ 0, %{name}_core_ok ], [ %{name}_stage_next, %{name}_stage_byte ]\n  %{name}_all_validated = icmp eq i32 %{name}_validated, %{name}_capacity\n  br i1 %{name}_all_validated, label %{name}_staged, label %{name}_validate\n{name}_validate:\n  %{name}_validated64 = zext i32 %{name}_validated to i64\n  %{name}_cell_ptr = getelementptr i32, ptr %{name}_buffer, i64 %{name}_validated64\n  %{name}_cell = load i32, ptr %{name}_cell_ptr, align 1\n  %{name}_cell_valid = icmp ule i32 %{name}_cell, 255\n  br i1 %{name}_cell_valid, label %{name}_stage_byte, label %{name}_invalid_input\n{name}_stage_byte:\n  %{name}_stage_ptr = getelementptr i8, ptr %output_scratch, i64 %{name}_validated64\n  %{name}_byte_value = trunc i32 %{name}_cell to i8\n  store i8 %{name}_byte_value, ptr %{name}_stage_ptr, align 1\n  %{name}_stage_next = add i32 %{name}_validated, 1\n  br label %{name}_stage_check\n{name}_staged:\n  br label %{name}_write_check\n{name}_write_check:\n  %{name}_accepted = phi i32 [ 0, %{name}_staged ], [ %{name}_next, %{name}_byte ], [ %{name}_accepted, %{name}_retry ]\n  %{name}_at_capacity = icmp eq i32 %{name}_accepted, %{name}_capacity\n  br i1 %{name}_at_capacity, label %{name}_complete, label %{name}_attempt\n{name}_attempt:", buffer.0).unwrap();
    emit_guard(out, diagnostics, &format!("{name}_write"), 1, span);
    // Exactly one attempt per debit, including EINTR, zero progress and every
    // error. Only accepted bytes advance the exact progress prefix.
    writeln!(out, "  %{name}_accepted64 = zext i32 %{name}_accepted to i64\n  %{name}_byte_ptr = getelementptr i8, ptr %output_scratch, i64 %{name}_accepted64\n  %{name}_status = call i32 @__oxid_write_stdout_byte(ptr %{name}_byte_ptr)\n  switch i32 %{name}_status, label %{name}_io_error [\n    i32 1, label %{name}_byte\n    i32 -1, label %{name}_retry\n  ]\n{name}_byte:\n  %{name}_next = add i32 %{name}_accepted, 1\n  br label %{name}_write_check\n{name}_retry:\n  br label %{name}_write_check\n{name}_complete:").unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    writeln!(
        out,
        "  store i32 {}, ptr {result}, align 1\n  br label %{name}_{suffix}\n{name}_invalid_input:",
        complete.tag()
    )
    .unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    // Nullary results never touch inactive payload bytes or padding.
    writeln!(
        out,
        "  store i32 {}, ptr {result}, align 1\n  br label %{name}_{suffix}\n{name}_io_error:",
        invalid_input.tag()
    )
    .unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    writeln!(out, "  store i32 {}, ptr {result}, align 1", io_error.tag()).unwrap();
    if !out.expand(Expansion::Construct) {
        return;
    }
    enum_payload_store(
        out,
        &format!("{name}_io_error_result"),
        &result,
        io_error.payload().expect("verified IoError payload"),
        &format!("%{name}_accepted"),
        io_error.payload_offset().expect("verified IoError offset"),
    );
    writeln!(out, "  br label %{name}_{suffix}\n{name}_{suffix}:").unwrap();
}

fn emit_statement(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let id = storage.id();
    if out.exceeded {
        return;
    }
    out.ordinary_visits += 1;
    let f = &plan.witness().functions()[id.0];
    match &statement.kind {
        OwnedInstruction::Experimental {
            operation,
            destination,
        } => experimental::operation(
            storage,
            name,
            *operation,
            *destination,
            diagnostics,
            plan::instruction_span(statement),
            out,
        ),
        OwnedInstruction::ReadStdin {
            buffer,
            destination,
        } => emit_read_stdin(
            storage,
            name,
            (*buffer, *destination),
            (diagnostics, plan::instruction_span(statement)),
            out,
        ),
        OwnedInstruction::WriteStdout {
            buffer,
            destination,
        } => emit_write_stdout(
            storage,
            name,
            (*buffer, *destination),
            (diagnostics, plan::instruction_span(statement)),
            out,
        ),
        OwnedInstruction::ConstructOwnedEnum {
            destination,
            variant,
            payload,
        } => {
            let declaration = plan
                .witness()
                .declarations()
                .enums()
                .variant(variant.enumeration, *variant)
                .expect("verified owned constructor");
            let output = field_pointer(
                out,
                &format!("{name}_owned_out"),
                &format!("%o{}", destination.0),
                declaration.payload_offset().expect("owned payload offset"),
            );
            active_transfer(
                out,
                plan,
                &format!("{name}_owned"),
                f.owners[payload.0].aggregate(),
                &format!("%o{}", payload.0),
                Some(&output),
                (diagnostics, plan::instruction_span(statement)),
            );
            if !out.expand(Expansion::Construct) {
                return;
            }
            writeln!(
                out,
                "  store i32 {}, ptr %o{}, align 1\n  br label %{name}_enum_ok\n{name}_enum_ok:",
                declaration.tag(),
                destination.0
            )
            .unwrap();
        }
        OwnedInstruction::ConsumeOwnedVariant {
            match_id,
            arm,
            destination,
        } => {
            let descriptor = &f.matches[match_id.0];
            let selected = descriptor.arms[*arm];
            let declaration = plan
                .witness()
                .declarations()
                .enums()
                .variant(selected.variant.enumeration, selected.variant)
                .expect("verified owned variant");
            let source = format!("%o{}", descriptor.source.0);
            writeln!(out, "  %{name}_tag = load i32, ptr {source}, align 1\n  %{name}_selected = icmp eq i32 %{name}_tag, {}\n  br i1 %{name}_selected, label %{name}_tag_valid, label %{name}_tag_error\n{name}_tag_error:", declaration.tag()).unwrap();
            emit_failure(out, diagnostics, FailureKind::EnumTag, descriptor.span);
            writeln!(out, "{name}_tag_valid:").unwrap();
            let input = field_pointer(
                out,
                &format!("{name}_owned_in"),
                &source,
                declaration.payload_offset().expect("owned payload offset"),
            );
            active_transfer(
                out,
                plan,
                &format!("{name}_owned"),
                f.owners[destination.0].aggregate(),
                &input,
                Some(&format!("%o{}", destination.0)),
                (diagnostics, descriptor.span),
            );
            writeln!(out, "  br label %{name}_enum_ok\n{name}_enum_ok:").unwrap();
        }
        OwnedInstruction::ConstructEnum {
            destination,
            variant,
            payload,
        } => {
            let AggregateTy::Enum(enumeration) = f.owners[destination.0].aggregate() else {
                unreachable!("verified enum constructor")
            };
            let declaration = plan
                .witness()
                .declarations()
                .enums()
                .variant(enumeration, *variant)
                .expect("verified constructor variant");
            // Scalar operands are already evaluated. Read before the first store.
            let value =
                payload.map(|operand| load_operand(out, f, &format!("{name}_payload"), operand));
            if !out.expand(Expansion::Construct) {
                return;
            }
            writeln!(
                out,
                "  store i32 {}, ptr %o{}, align 1",
                declaration.tag(),
                destination.0
            )
            .unwrap();
            if let Some(value) = value {
                if !out.expand(Expansion::Construct) {
                    return;
                }
                enum_payload_store(
                    out,
                    name,
                    &format!("%o{}", destination.0),
                    declaration.payload().expect("verified payload"),
                    &value,
                    declaration.payload_offset().expect("active scalar offset"),
                );
            }
        }
        OwnedInstruction::ConsumeVariant {
            match_id,
            arm,
            destination,
        } => {
            let descriptor = &f.matches[match_id.0];
            let selected = descriptor.arms[*arm];
            let AggregateTy::Enum(enumeration) = f.owners[descriptor.source.0].aggregate() else {
                unreachable!("verified enum consume source")
            };
            let variant = plan
                .witness()
                .declarations()
                .enums()
                .variant(enumeration, selected.variant)
                .expect("verified consume variant");
            let source = format!("%o{}", descriptor.source.0);
            writeln!(out, "  %{name}_tag = load i32, ptr {source}, align 1\n  %{name}_selected = icmp eq i32 %{name}_tag, {}\n  br i1 %{name}_selected, label %{name}_tag_valid, label %{name}_tag_error\n{name}_tag_error:", variant.tag()).unwrap();
            emit_failure(out, diagnostics, FailureKind::EnumTag, descriptor.span);
            writeln!(out, "{name}_tag_valid:").unwrap();
            let active = enum_payload_load(
                out,
                plan,
                name,
                &source,
                selected.variant,
                (diagnostics, descriptor.span),
            );
            if let Some((scalar, value)) = active {
                store_slot(
                    out,
                    &format!("{name}_bind"),
                    &format!("%s{}", destination.expect("verified payload destination").0),
                    scalar,
                    &value,
                );
            }
            writeln!(out, "  br label %{name}_enum_ok\n{name}_enum_ok:").unwrap();
        }
        OwnedInstruction::Discard(owner) => {
            let aggregate = f.owners[owner.0].aggregate();
            if resources::contains(plan, aggregate) {
                resources::drop_owner(
                    out,
                    plan,
                    name,
                    aggregate,
                    &format!("%o{}", owner.0),
                    (diagnostics, plan::instruction_span(statement)),
                );
            } else if let AggregateTy::Enum(enumeration) = aggregate {
                enum_transfer(
                    out,
                    plan,
                    name,
                    enumeration,
                    &format!("%o{}", owner.0),
                    None,
                    (diagnostics, plan::instruction_span(statement)),
                );
            }
        }
        OwnedInstruction::ConstructArray {
            destination,
            elements,
        } => {
            let AggregateTy::FixedArray(array) = f.owners[destination.0].aggregate() else {
                unreachable!("verified array constructor")
            };
            if elements.is_empty() {
                if !out.expand(Expansion::Construct) {
                    return;
                }
                writeln!(
                    out,
                    "  store {} 0, ptr %o{}, align 1",
                    sentinel_ty(array),
                    destination.0
                )
                .unwrap();
            }
            for (i, value) in elements.iter().enumerate() {
                if !out.expand(Expansion::Construct) {
                    return;
                }
                let n = format!("{name}_element{i}");
                let value = load_operand(out, f, &format!("{n}_value"), *value);
                let offset = i
                    .checked_mul(array.stride())
                    .expect("verified array offset");
                let ptr = field_pointer(out, &n, &format!("%o{}", destination.0), offset);
                writeln!(
                    out,
                    "  store {} {value}, ptr {ptr}, align 1",
                    ty(array.element())
                )
                .unwrap();
            }
        }
        OwnedInstruction::ReadIndex { destination, .. }
        | OwnedInstruction::ReadProjection {
            destination,
            index: Some(_),
            ..
        } => {
            let (element, ptr) = index_pointer(plan, id, name, statement, diagnostics, out);
            writeln!(
                out,
                "  %{name}_value = load {}, ptr {ptr}, align 1",
                ty(element)
            )
            .unwrap();
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", destination.0),
                element,
                &format!("%{name}_value"),
            );
        }
        OwnedInstruction::WriteIndex { value, .. }
        | OwnedInstruction::WriteProjection {
            value,
            index: Some(_),
            ..
        } => {
            let (element, ptr) = index_pointer(plan, id, name, statement, diagnostics, out);
            // The operand is the scalar snapshot made before the index helper;
            // it is never recomputed from the mutated aggregate.
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            writeln!(out, "  store {} {value}, ptr {ptr}, align 1", ty(element)).unwrap();
        }
        OwnedInstruction::ArrayLength { destination, base } => {
            let length = index_length(out, f, name, *base);
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", destination.0),
                hir::Ty::I32,
                &length,
            );
        }
        OwnedInstruction::ProjectionLength {
            destination,
            base,
            path,
        } => {
            let (value, _) = projection(plan, f, *base, path);
            let ValueTy::Owned(AggregateTy::FixedArray(array)) = value else {
                unreachable!("verified projected array length")
            };
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", destination.0),
                hir::Ty::I32,
                &array.length().to_string(),
            );
        }
        OwnedInstruction::ReadProjection {
            destination,
            base,
            path,
            index: None,
        } => {
            let (value, offset) = projection(plan, f, *base, path);
            let ValueTy::Scalar(scalar) = value else {
                unreachable!("verified scalar projection")
            };
            let base = base_pointer(out, name, *base);
            let ptr = field_pointer(out, name, &base, offset);
            writeln!(
                out,
                "  %{name}_value = load {}, ptr {ptr}, align 1",
                ty(scalar)
            )
            .unwrap();
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", destination.0),
                scalar,
                &format!("%{name}_value"),
            );
        }
        OwnedInstruction::WriteProjection {
            base,
            path,
            index: None,
            value,
        } => {
            let (field_ty, offset) = projection(plan, f, *base, path);
            let ValueTy::Scalar(scalar) = field_ty else {
                unreachable!("verified scalar projection")
            };
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            let base = base_pointer(out, name, *base);
            let ptr = field_pointer(out, name, &base, offset);
            writeln!(out, "  store {} {value}, ptr {ptr}, align 1", ty(scalar)).unwrap();
        }
        OwnedInstruction::DestructureRecord { source, fields } => {
            // Admit every field before the first ownership-bearing store.
            for (index, (field, destination)) in fields.iter().enumerate() {
                let member = plan
                    .witness()
                    .declarations()
                    .field(field.record, *field)
                    .expect("verified record pattern field");
                let name = format!("{name}_admit{index}");
                let pointer =
                    field_pointer(out, &name, &format!("%o{}", source.0), member.offset());
                active_transfer_with_intent(
                    out,
                    plan,
                    &name,
                    f.owners[destination.0].aggregate(),
                    &pointer,
                    TransferIntent::Validate,
                    (diagnostics, plan::instruction_span(statement)),
                );
            }
            for (index, (field, destination)) in fields.iter().enumerate() {
                let member = plan
                    .witness()
                    .declarations()
                    .field(field.record, *field)
                    .expect("verified record pattern field");
                let name = format!("{name}_move{index}");
                let pointer =
                    field_pointer(out, &name, &format!("%o{}", source.0), member.offset());
                active_transfer_with_intent(
                    out,
                    plan,
                    &name,
                    f.owners[destination.0].aggregate(),
                    &pointer,
                    TransferIntent::Move(&format!("%o{}", destination.0)),
                    (diagnostics, plan::instruction_span(statement)),
                );
            }
            if !out.expand(Expansion::Transfer) {
                return;
            }
            writeln!(out, "  br label %{name}_aggregate_ok\n{name}_aggregate_ok:").unwrap();
        }
        OwnedInstruction::ConstructComposite {
            destination,
            fields,
        } => {
            if resources::contains(plan, f.owners[destination.0].aggregate()) {
                for (index, (_, value)) in fields.iter().enumerate() {
                    if let FieldInitializer::Owned(source) = value {
                        active_transfer_with_intent(
                            out,
                            plan,
                            &format!("{name}_incoming{index}"),
                            f.owners[source.0].aggregate(),
                            &format!("%o{}", source.0),
                            TransferIntent::Validate,
                            (diagnostics, plan::instruction_span(statement)),
                        );
                    }
                }
            }
            if fields.is_empty() {
                if !out.expand(Expansion::Construct) {
                    return;
                }
                writeln!(out, "  store i8 0, ptr %o{}, align 1", destination.0).unwrap();
            }
            for (i, (field, value)) in fields.iter().enumerate() {
                if out.exceeded {
                    return;
                }
                let field = plan
                    .witness()
                    .declarations()
                    .field(field.record, *field)
                    .expect("verified composite field");
                let n = format!("{name}_field{i}");
                match *value {
                    FieldInitializer::Scalar(value) => {
                        if !out.expand(Expansion::Construct) {
                            return;
                        }
                        #[cfg(test)]
                        {
                            out.field_visits += 1;
                        }
                        let ValueTy::Scalar(scalar) = field.value_ty() else {
                            unreachable!("verified scalar initializer")
                        };
                        let value = load_operand(out, f, &format!("{n}_value"), value);
                        let ptr =
                            field_pointer(out, &n, &format!("%o{}", destination.0), field.offset());
                        writeln!(out, "  store {} {value}, ptr {ptr}, align 1", ty(scalar))
                            .unwrap();
                    }
                    FieldInitializer::Owned(source) => {
                        // Each child is a complete, already-evaluated staging owner.
                        // No recursive native calls or intermediate leaf buffers.
                        let ptr =
                            field_pointer(out, &n, &format!("%o{}", destination.0), field.offset());
                        transfer_leaves(
                            out,
                            plan,
                            &n,
                            f.owners[source.0].aggregate(),
                            (&format!("%o{}", source.0), &ptr),
                            Expansion::Construct,
                            (diagnostics, plan::instruction_span(statement)),
                        );
                    }
                }
            }
        }
        OwnedInstruction::Scalar(_) => emit_scalar(storage, name, statement, diagnostics, out),
        OwnedInstruction::Construct {
            destination,
            fields,
        } => {
            if fields.is_empty() {
                if !out.expand(Expansion::Construct) {
                    return;
                }
                writeln!(out, "  store i8 0, ptr %o{}, align 1", destination.0).unwrap();
            }
            for (i, (field, value)) in fields.iter().enumerate() {
                if !out.expand(Expansion::Construct) {
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
            if matches!(statement.kind, OwnedInstruction::Replace { .. }) {
                if resources::contains(plan, f.owners[destination.0].aggregate()) {
                    active_transfer_with_intent(
                        out,
                        plan,
                        &format!("{name}_incoming"),
                        f.owners[source.0].aggregate(),
                        &format!("%o{}", source.0),
                        TransferIntent::Validate,
                        (diagnostics, plan::instruction_span(statement)),
                    );
                }
                resources::drop_owner(
                    out,
                    plan,
                    &format!("{name}_old"),
                    f.owners[destination.0].aggregate(),
                    &format!("%o{}", destination.0),
                    (diagnostics, plan::instruction_span(statement)),
                );
            }
            transfer_checked(
                out,
                plan,
                name,
                f.owners[source.0].aggregate(),
                (&format!("%o{}", source.0), &format!("%o{}", destination.0)),
                (diagnostics, plan::instruction_span(statement)),
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
            let slot = storage.argument_slot(*call, *argument);
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
            transfer_checked(
                out,
                plan,
                name,
                f.owners[source.0].aggregate(),
                (&format!("%o{}", source.0), &format!("%o{}", destination.0)),
                (diagnostics, plan::instruction_span(statement)),
            );
        }
        OwnedInstruction::PrepareBorrowRange {
            loan, start, end, ..
        } => experimental::borrow(
            storage,
            name,
            *loan,
            Some((*start, *end)),
            diagnostics,
            plan::instruction_span(statement),
            out,
        ),
        OwnedInstruction::PrepareBorrow { loan, .. } => {
            if experimental::resource_borrow(plan, f, *loan) {
                experimental::borrow(
                    storage,
                    name,
                    *loan,
                    None,
                    diagnostics,
                    plan::instruction_span(statement),
                    out,
                );
                return;
            }
            let declaration = &f.loans[loan.0];
            let base = base_pointer(out, name, declaration.authority);
            if out.exceeded {
                return;
            }
            let (pointer, projected_length) = if declaration.projection.is_empty() {
                (base, None)
            } else {
                // A checked path selects a view, never independent authority.
                // Resolve against the witness's nominal root and retain only
                // its address and actual array length in the existing slots.
                out.borrow_projection_visits += declaration.projection.len();
                let (value, offset) =
                    projection(plan, f, declaration.authority, &declaration.projection);
                let projected_length = match value {
                    ValueTy::Owned(AggregateTy::FixedArray(array)) => Some(array.length()),
                    ValueTy::Owned(AggregateTy::Record(_)) => None,
                    _ => unreachable!("verified projected record or array slice"),
                };
                (
                    field_pointer(out, &format!("{name}_borrow_projection"), &base, offset),
                    projected_length,
                )
            };
            writeln!(
                out,
                "  store ptr {pointer}, ptr %r{}, align 8",
                storage.loan_slot(*loan)
            )
            .unwrap();
            if matches!(declaration.referent(), BorrowedTy::ScalarSlice(_)) {
                // Stage the target view's length when its loan begins, alongside
                // the pointer, before evaluating any later call argument.
                let length = projected_length.map_or_else(
                    || index_length(out, f, name, declaration.authority),
                    |length| length.to_string(),
                );
                writeln!(out, "  store i32 {length}, ptr %ll{}, align 4", loan.0).unwrap();
            }
        }
        // The witness proves these logical transitions. Backing storage remains
        // allocated throughout the activation, including suspended staging.
        OwnedInstruction::StorageLive(owner) => {
            resources::initialize(
                out,
                plan,
                f.owners[owner.0].aggregate(),
                &format!("%o{}", owner.0),
            );
        }
        OwnedInstruction::StorageEnd(owner) => {
            resources::drop_owner(
                out,
                plan,
                name,
                f.owners[owner.0].aggregate(),
                &format!("%o{}", owner.0),
                (diagnostics, plan::instruction_span(statement)),
            );
        }
        OwnedInstruction::OpenCall(_) => {}
    }
}
fn emit_scalar(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    owned_statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    let plan = storage.execution();
    let id = storage.id();
    if out.exceeded {
        return;
    }
    out.ordinary_visits += 1;
    let OwnedInstruction::Scalar(statement) = &owned_statement.kind else {
        unreachable!("scalar operation")
    };
    let f = &plan.witness().functions()[id.0];
    let assign = match statement {
        Statement::Assign(a) => a,
        Statement::Initialize { place, value, .. } | Statement::Store { place, value, .. } => {
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", storage.place_slot(place.id)),
                f.places[place.id.0].ty,
                &value,
            );
            return;
        }
    };
    let t = f.locals[assign.destination.0].ty;
    let value = match assign.value {
        Rvalue::CharacterConvert {
            op,
            operand,
            name_span,
            ..
        } => {
            let input = f.locals[operand.local.0].ty;
            let value = load_operand(out, f, &format!("{name}_source"), operand);
            if op == hir::ConversionOp::CharToU32 {
                value
            } else {
                let width = ty(input);
                let guarded = if input.quad_integer() {
                    value.clone()
                } else {
                    let extension = if input.signed_integer() {
                        "sext"
                    } else {
                        "zext"
                    };
                    writeln!(
                        out,
                        "  %{name}_char_full = {extension} {width} {value} to i128"
                    )
                    .unwrap();
                    format!("%{name}_char_full")
                };
                writeln!(out, "  %{name}_char_max = icmp ule i128 {guarded}, 1114111\n  %{name}_char_before = icmp ult i128 {guarded}, 55296\n  %{name}_char_after = icmp ugt i128 {guarded}, 57343\n  %{name}_char_not_surrogate = or i1 %{name}_char_before, %{name}_char_after\n  %{name}_char_valid = and i1 %{name}_char_max, %{name}_char_not_surrogate\n  br i1 %{name}_char_valid, label %{name}_conversion_ok, label %{name}_char_error\n{name}_char_error:").unwrap();
                emit_failure(out, diagnostics, FailureKind::CharacterRange, name_span);
                writeln!(out, "{name}_conversion_ok:").unwrap();
                if input.integer_bits() == Some(32) {
                    value
                } else {
                    let instruction = if input.integer_bits().expect("verified integer") < 32 {
                        "zext"
                    } else {
                        "trunc"
                    };
                    writeln!(
                        out,
                        "  %{name}_value = {instruction} {width} {value} to i32"
                    )
                    .unwrap();
                    format!("%{name}_value")
                }
            }
        }
        Rvalue::Character(value) => u32::from(value).to_string(),
        Rvalue::Float { bits, ty: selected } => {
            let integer = if selected == hir::Ty::F32 {
                "i32"
            } else {
                "i64"
            };
            writeln!(
                out,
                "  %{name}_value = bitcast {integer} {bits} to {}",
                ty(selected)
            )
            .unwrap();
            format!("%{name}_value")
        }
        Rvalue::FloatNegate { operand, .. } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            writeln!(out, "  %{name}_value = fneg {} {value}", ty(t)).unwrap();
            format!("%{name}_value")
        }
        Rvalue::FloatArithmetic {
            op, left, right, ..
        } => {
            let a = load_operand(out, f, &format!("{name}_left"), left);
            let b = load_operand(out, f, &format!("{name}_right"), right);
            floats::arithmetic(out, name, t, op, &a, &b)
        }
        Rvalue::FloatConvert { op, operand, .. } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            floats::convert(out, name, f.locals[operand.local.0].ty, t, op, &value)
        }
        Rvalue::BitNot { operand, .. } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            writeln!(out, "  %{name}_value = xor {} {value}, -1", ty(t)).unwrap();
            format!("%{name}_value")
        }
        Rvalue::Bitwise {
            op,
            left,
            right,
            operator_span,
        } => {
            let a = load_operand(out, f, &format!("{name}_left"), left);
            let b = load_operand(out, f, &format!("{name}_right"), right);
            bitwise::binary(
                out,
                diagnostics,
                name,
                op,
                t,
                f.locals[right.local.0].ty,
                &a,
                &b,
                operator_span,
            )
        }
        Rvalue::Integer128 { low, high, .. } => {
            (u128::from(low) | (u128::from(high) << 64)).to_string()
        }
        Rvalue::Integer { ty, bits } => integers::literal(ty, bits),
        Rvalue::CheckedInteger {
            op,
            left,
            right,
            operator_span,
        } => {
            let a = load_operand(out, f, &format!("{name}_left"), left);
            let b = load_operand(out, f, &format!("{name}_right"), right);
            integers::arithmetic(out, diagnostics, name, t, op, &a, &b, operator_span)
        }
        Rvalue::CheckedNegateInteger {
            operand,
            operator_span,
        } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            integers::negate(out, diagnostics, name, t, &value, operator_span)
        }
        Rvalue::CheckedIntegerCast {
            operand,
            target,
            name_span,
            ..
        } => {
            let input = f.locals[operand.local.0].ty;
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            integers::cast(out, diagnostics, name, input, target, &value, name_span)
        }
        Rvalue::I64(value) => value.to_string(),
        Rvalue::WidenI64 { operand, .. } => {
            let input = f.locals[operand.local.0].ty;
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            numeric::widen(out, name, input, &value)
        }
        Rvalue::CheckedI64ToI32 {
            operand, name_span, ..
        }
        | Rvalue::CheckedI64ToU8 {
            operand, name_span, ..
        } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            numeric::narrow(out, diagnostics, name, t, &value, name_span)
        }
        Rvalue::CheckedNegateI64 {
            operand,
            operator_span,
        } => {
            let value = load_operand(out, f, &format!("{name}_operand"), operand);
            numeric::negate(out, diagnostics, name, &value, operator_span)
        }
        Rvalue::CheckedI64 {
            op,
            left,
            right,
            operator_span,
        } => {
            let a = load_operand(out, f, &format!("{name}_left"), left);
            let b = load_operand(out, f, &format!("{name}_right"), right);
            numeric::arithmetic(out, diagnostics, name, op, &a, &b, operator_span)
        }
        Rvalue::Bool(v) => if v { "true" } else { "false" }.to_string(),
        Rvalue::I32(v) => v.to_string(),
        Rvalue::U8(v) => v.to_string(),
        Rvalue::Unit => "0".into(),
        Rvalue::Copy(v) => load_operand(out, f, &format!("{name}_value"), v),
        Rvalue::Load(p) => load_slot(
            out,
            &format!("{name}_value"),
            &format!("%s{}", storage.place_slot(p.id)),
            t,
        ),
        Rvalue::CheckedI32ToU8 {
            operand, name_span, ..
        } => {
            let operand = load_operand(out, f, &format!("{name}_operand"), operand);
            let suffix = continuation(f, &owned_statement.kind)
                .suffix()
                .expect("conversion continuation");
            writeln!(out, "  %{name}_negative = icmp slt i32 {operand}, 0\n  %{name}_large = icmp sgt i32 {operand}, 255\n  %{name}_invalid = or i1 %{name}_negative, %{name}_large\n  br i1 %{name}_invalid, label %{name}_conversion_error, label %{name}_{suffix}\n{name}_conversion_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::ByteRange, name_span);
            writeln!(
                out,
                "{name}_{suffix}:\n  %{name}_value = trunc i32 {operand} to i8"
            )
            .unwrap();
            format!("%{name}_value")
        }
        Rvalue::U8ToI32 { operand, .. } => {
            let operand = load_operand(out, f, &format!("{name}_operand"), operand);
            writeln!(out, "  %{name}_value = zext i8 {operand} to i32").unwrap();
            format!("%{name}_value")
        }
        Rvalue::NotBool { operand, .. } => {
            let operand = load_operand(out, f, &format!("{name}_operand"), operand);
            writeln!(out, "  %{name}_value = xor i1 {operand}, true").unwrap();
            format!("%{name}_value")
        }
        Rvalue::CompareScalar {
            op, left, right, ..
        } => {
            let unsigned = matches!(
                f.locals[left.local.0].ty,
                hir::Ty::U8
                    | hir::Ty::U16
                    | hir::Ty::U32
                    | hir::Ty::U64
                    | hir::Ty::Usize
                    | hir::Ty::U128
            );
            let floating = f.locals[left.local.0].ty.floating();
            let operand_type = ty(f.locals[left.local.0].ty);
            let left = load_operand(out, f, &format!("{name}_left"), left);
            let right = load_operand(out, f, &format!("{name}_right"), right);
            let predicate = match op {
                hir::ComparisonOp::Equal => "eq",
                hir::ComparisonOp::NotEqual => "ne",
                hir::ComparisonOp::Less => {
                    if unsigned {
                        "ult"
                    } else {
                        "slt"
                    }
                }
                hir::ComparisonOp::LessEqual => {
                    if unsigned {
                        "ule"
                    } else {
                        "sle"
                    }
                }
                hir::ComparisonOp::Greater => {
                    if unsigned {
                        "ugt"
                    } else {
                        "sgt"
                    }
                }
                hir::ComparisonOp::GreaterEqual => {
                    if unsigned {
                        "uge"
                    } else {
                        "sge"
                    }
                }
            };
            let (instruction, predicate) = if floating {
                (
                    "fcmp",
                    match op {
                        hir::ComparisonOp::Equal => "oeq",
                        hir::ComparisonOp::NotEqual => "une",
                        hir::ComparisonOp::Less => "olt",
                        hir::ComparisonOp::LessEqual => "ole",
                        hir::ComparisonOp::Greater => "ogt",
                        hir::ComparisonOp::GreaterEqual => "oge",
                    },
                )
            } else {
                ("icmp", predicate)
            };
            writeln!(
                out,
                "  %{name}_value = {instruction} {predicate} {operand_type} {left}, {right}"
            )
            .unwrap();
            format!("%{name}_value")
        }
        Rvalue::CheckedNegateI32 {
            operand,
            operator_span,
        } => {
            let operand = load_operand(out, f, &format!("{name}_operand"), operand);
            let suffix = continuation(f, &owned_statement.kind)
                .suffix()
                .expect("arithmetic continuation");
            writeln!(out, "  %{name}_checked = call {{ i32, i1 }} @llvm.ssub.with.overflow.i32(i32 0, i32 {operand})\n  %{name}_overflow = extractvalue {{ i32, i1 }} %{name}_checked, 1").unwrap();
            writeln!(out, "  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_{suffix}\n{name}_checked_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::Overflow, operator_span);
            writeln!(
                out,
                "{name}_{suffix}:\n  %{name}_value = extractvalue {{ i32, i1 }} %{name}_checked, 0"
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
            let suffix = continuation(f, &owned_statement.kind)
                .suffix()
                .expect("arithmetic continuation");
            match op {
                hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
                    // Keep both undefined LLVM operand pairs outside the block
                    // that evaluates sdiv/srem, including MIN%-1 overflow.
                    writeln!(out, "  %{name}_zero = icmp eq i32 {right}, 0\n  br i1 %{name}_zero, label %{name}_division_error, label %{name}_nonzero\n{name}_division_error:").unwrap();
                    emit_failure(out, diagnostics, FailureKind::DivisionByZero, operator_span);
                    writeln!(out, "{name}_nonzero:\n  %{name}_min = icmp eq i32 {left}, -2147483648\n  %{name}_negative_one = icmp eq i32 {right}, -1\n  %{name}_overflow = and i1 %{name}_min, %{name}_negative_one").unwrap();
                }
                hir::ArithmeticOp::Add
                | hir::ArithmeticOp::Subtract
                | hir::ArithmeticOp::Multiply => {
                    let intrinsic = match op {
                        hir::ArithmeticOp::Add => "sadd",
                        hir::ArithmeticOp::Subtract => "ssub",
                        hir::ArithmeticOp::Multiply => "smul",
                        _ => unreachable!("overflow intrinsic"),
                    };
                    writeln!(out, "  %{name}_checked = call {{ i32, i1 }} @llvm.{intrinsic}.with.overflow.i32(i32 {left}, i32 {right})\n  %{name}_overflow = extractvalue {{ i32, i1 }} %{name}_checked, 1").unwrap();
                }
            }
            writeln!(out, "  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_{suffix}\n{name}_checked_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::Overflow, operator_span);
            writeln!(out, "{name}_{suffix}:").unwrap();
            match op {
                hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
                    let instruction = if op == hir::ArithmeticOp::Divide {
                        "sdiv"
                    } else {
                        "srem"
                    };
                    writeln!(out, "  %{name}_value = {instruction} i32 {left}, {right}").unwrap();
                }
                hir::ArithmeticOp::Add
                | hir::ArithmeticOp::Subtract
                | hir::ArithmeticOp::Multiply => {
                    writeln!(
                        out,
                        "  %{name}_value = extractvalue {{ i32, i1 }} %{name}_checked, 0"
                    )
                    .unwrap();
                }
            }
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
fn frame_usage(plan: &ExecutionPlan<'_>, function: hir::DefId) -> hosted_frames::Usage {
    let usage = plan.function(function).usage();
    hosted_frames::Usage {
        slots: usage.scalar_slots,
        bytes: usage.native_bytes,
    }
}

fn emit_frame_enter(
    out: &mut Emission,
    diagnostics: &Diagnostics,
    name: &str,
    usage: hosted_frames::Usage,
    span: Span,
) {
    hosted_frames::enter(out, name, usage, |out, kind| {
        let kind = match kind {
            hosted_frames::Failure::Frames => FailureKind::Frames,
            hosted_frames::Failure::Slots => FailureKind::Slots,
            hosted_frames::Failure::Bytes => FailureKind::NativeFrameBytes,
        };
        emit_failure(out, diagnostics, kind, span);
    });
}

fn emit_terminator(
    storage: &NativeFunctionStorage<'_, '_>,
    name: &str,
    term: &OwnedTerminator,
    context: (&Diagnostics, bool),
    out: &mut Emission,
) {
    let plan = storage.execution();
    let id = storage.id();
    if out.exceeded {
        return;
    }
    out.ordinary_visits += 1;
    let f = &plan.witness().functions()[id.0];
    let (diagnostics, guarded) = context;
    match &term.kind {
        OwnedTerminatorKind::MatchDispatch { match_id, arm } => {
            emit_dispatch(out, plan, f, name, *match_id, *arm, diagnostics);
        }
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
            resources::drop_frame(out, plan, f, name, (diagnostics, term.span));
            writeln!(out, "  ret {t} {value}").unwrap();
        }
        OwnedTerminatorKind::ReturnOwned(owner) => {
            if plan.binary_delivery_cost(id).is_some() || plan.file_delivery_cost(id).is_some() {
                // The canonical operation prepaid this exact return cost and
                // already delivered into %result. It owns no resource leaves.
                out.write_str("  ret void\n").unwrap();
                return;
            }
            transfer_checked(
                out,
                plan,
                name,
                f.owners[owner.0].aggregate(),
                (&format!("%o{}", owner.0), "%result"),
                (diagnostics, term.span),
            );
            resources::drop_frame(out, plan, f, name, (diagnostics, term.span));
            out.write_str("  ret void\n").unwrap();
        }
        OwnedTerminatorKind::Invoke { call, continuation } => {
            let descriptor = &f.calls[call.0];
            let callee = &plan.witness().functions()[descriptor.target.0];
            // Bounded by MAX_PARAMS; argument strings are emission scratch only,
            // never runtime A-sized scalar or owned result scratch.
            let mut args =
                Vec::with_capacity(descriptor.arguments.len() + slice_reference_count(callee) + 2);
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
                out.ordinary_visits += 1;
                match argument {
                    ArgumentSlot::Scalar => {
                        let ParameterBinding::Scalar(l) = callee.parameters[i] else {
                            unreachable!("verified scalar parameter")
                        };
                        let t = callee.locals[l.0].ty;
                        let slot = storage.argument_slot(*call, i);
                        let value =
                            load_slot(out, &format!("{name}_arg{i}"), &format!("%s{slot}"), t);
                        args.push(format!("{} {value}", ty(t)));
                    }
                    ArgumentSlot::Owned(o) => args.push(format!("ptr %o{}", o.0)),
                    ArgumentSlot::Borrow(l) => {
                        writeln!(
                            out,
                            "  %{name}_arg{i} = load ptr, ptr %r{}, align 8",
                            storage.loan_slot(*l)
                        )
                        .unwrap();
                        args.push(format!("ptr %{name}_arg{i}"));
                        if matches!(f.loans[l.0].referent(), BorrowedTy::ScalarSlice(_)) {
                            writeln!(
                                out,
                                "  %{name}_arg{i}_length = load i32, ptr %ll{}, align 4",
                                l.0
                            )
                            .unwrap();
                            args.push(format!("i32 %{name}_arg{i}_length"));
                        }
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
            let usage = frame_usage(plan, descriptor.target);
            if out.policy.recursive() {
                emit_frame_enter(out, diagnostics, name, usage, term.span);
            }
            writeln!(
                out,
                "  {prefix}call {} @__oxid_owned_fn_{}({})",
                result_ty(callee.result),
                descriptor.target.0,
                joined
            )
            .unwrap();
            if out.policy.recursive() {
                hosted_frames::leave(out, name, usage);
            }
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
#[path = "builtin_input_native_tests.rs"]
mod builtin_input_tests;
#[cfg(test)]
#[path = "builtin_output_native_tests.rs"]
mod builtin_output_tests;
#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;

#[cfg(test)]
mod division_tests {
    use super::*;
    use crate::frontend::oir::owned::source::resource_fixtures;

    #[test]
    fn division_native_owned_guards_precede_instructions_and_keep_phi_exits() {
        for prefix in ["", "while false {}"] {
            let case = resource_fixtures::checked(&format!(
                "struct Number {{ value:i32 }} fn main()->bool {{ let x=Number {{ value:-7 }}; {prefix} return (x.value/3==-2) && (x.value%3==-1); }}"
            ));
            let module = native_module(&case.witness, Some(case.entry), &case.sources).unwrap();
            let f = &case.witness.functions()[case.entry.0];
            for (b, block) in f.blocks.iter().enumerate() {
                for (i, statement) in block.statements.iter().enumerate() {
                    let OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI32 { op, .. },
                        ..
                    })) = &statement.kind
                    else {
                        continue;
                    };
                    let name = format!("f{}_b{b}_i{i}", f.id.0);
                    let instruction = if *op == hir::ArithmeticOp::Divide {
                        "sdiv"
                    } else {
                        "srem"
                    };
                    let zero = module
                        .find(&format!("  %{name}_zero = icmp eq i32 "))
                        .unwrap();
                    let nonzero = module
                        .find(&format!("{name}_nonzero:\n  %{name}_min = icmp eq i32 "))
                        .unwrap();
                    let overflow = module
                        .find(&format!(
                            "  %{name}_overflow = and i1 %{name}_min, %{name}_negative_one"
                        ))
                        .unwrap();
                    let success = module
                        .find(&format!(
                            "{name}_checked_ok:\n  %{name}_value = {instruction} i32 "
                        ))
                        .unwrap();
                    assert!(zero < nonzero && nonzero < overflow && overflow < success);
                    assert!(module.contains(&format!(
                        "  br i1 %{name}_zero, label %{name}_division_error, label %{name}_nonzero"
                    )));
                    assert!(module.contains(&format!("  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok")));
                    assert!(module.contains(&format!("%{name}_right, -1")));
                }
                if let Some(merge) = &block.merge {
                    for input in merge.incoming {
                        let predecessor = &f.blocks[input.predecessor.0];
                        let label = if prefix.is_empty() {
                            predecessor
                                .statements
                                .iter()
                                .enumerate()
                                .rfind(|(_, s)| {
                                    matches!(
                                        s.kind,
                                        OwnedInstruction::Scalar(Statement::Assign(Assign {
                                            value: Rvalue::CheckedI32 { .. },
                                            ..
                                        }))
                                    )
                                })
                                .map_or_else(
                                    || format!("b{}", input.predecessor.0),
                                    |(i, _)| {
                                        format!(
                                            "f{}_b{}_i{i}_checked_ok",
                                            f.id.0, input.predecessor.0
                                        )
                                    },
                                )
                        } else {
                            format!(
                                "f{}_b{}_g{}_ok",
                                f.id.0,
                                input.predecessor.0,
                                predecessor.statements.len() + 1
                            )
                        };
                        assert!(module.lines().any(|line| line.contains(" = phi ptr ")
                            && line.contains(&format!(", %{label} ]"))));
                    }
                }
            }
            assert_eq!(module.matches(" = sdiv i32 ").count(), 1);
            assert_eq!(module.matches(" = srem i32 ").count(), 1);
            assert_eq!(
                execute::run(&case.witness, Some(case.entry)),
                Ok(Scalar::Bool(true))
            );
        }
    }

    #[test]
    fn division_native_owned_diagnostic_ledger_and_caps_include_zero_failures() {
        for prefix in ["", "while false {}"] {
            let case = resource_fixtures::checked(&format!(
                "struct Number {{ value:i32 }} fn main()->i32 {{ let x=Number {{ value:7 }}; {prefix} return (x.value/3)%2; }}"
            ));
            let plan = ExecutionPlan::build(&case.witness).unwrap();
            let diagnostics = Diagnostics::new(
                &plan,
                case.entry,
                &case.sources,
                !prefix.is_empty(),
                MAX_DIAGNOSTIC_BYTES,
            )
            .unwrap();
            for kind in [FailureKind::Overflow, FailureKind::DivisionByZero] {
                assert_eq!(
                    diagnostics
                        .ids
                        .iter()
                        .filter(|(key, _)| key.0 == kind)
                        .count(),
                    2
                );
                for &(key, id) in diagnostics.ids.iter().filter(|(key, _)| key.0 == kind) {
                    let span = Span {
                        file: crate::frontend::source::SourceFileId(key.1),
                        start: key.2,
                        end: key.3,
                    };
                    assert_eq!(
                        diagnostics.messages[id],
                        kind.diagnostic(span, &case.sources)
                            .render_human(&case.sources)
                    );
                }
            }
            let observation = run_array_observed(
                &case.witness,
                Some(case.entry),
                &case.sources,
                NativeControl::default(),
            );
            let module = observation.result.unwrap();
            let metrics = observation.metrics;
            let bytes = diagnostics.messages.iter().map(String::len).sum();
            assert_eq!(metrics.message_bytes, bytes);
            assert_eq!(metrics.count_bytes, module.len());
            assert_eq!(metrics.render_bytes, module.len());
            assert!(metrics.metadata_peak <= Limits::DEFAULT.metadata_bytes);
            assert!(metrics.metadata_admitted_bytes <= Limits::DEFAULT.metadata_bytes);
            assert_eq!(
                metrics.occurrence_bytes,
                metrics.occurrences * size_of::<DiagnosticOccurrence>()
            );
            assert_eq!(
                metrics.lookup_bytes,
                metrics.unique * size_of::<DiagnosticLookup>()
            );
            assert!(metrics.diagnostic_transient_peak <= DIAGNOSTIC_TRANSIENT_BYTES);
            if prefix.is_empty() {
                assert_eq!((metrics.occurrences, metrics.unique), (4, 4));
            }
            let limits = Limits {
                diagnostic_bytes: bytes,
                ir_bytes: module.len(),
                ..Limits::DEFAULT
            };
            assert_eq!(
                native_module_limits(
                    &case.witness,
                    Some(case.entry),
                    &case.sources,
                    plan::MAX_FUEL,
                    limits
                )
                .unwrap(),
                module
            );
            for (limits, marker) in [
                (
                    Limits {
                        diagnostic_bytes: bytes - 1,
                        ..limits
                    },
                    "diagnostic bytes",
                ),
                (
                    Limits {
                        ir_bytes: module.len() - 1,
                        ..limits
                    },
                    "LLVM bytes",
                ),
            ] {
                let error = native_module_limits(
                    &case.witness,
                    Some(case.entry),
                    &case.sources,
                    plan::MAX_FUEL,
                    limits,
                )
                .unwrap_err();
                assert_eq!(error.code, "E0700");
                assert!(error.message.contains(marker));
            }
        }
    }
}

#[cfg(test)]
#[path = "native_inventory_admission_tests.rs"]
mod inventory_admission_tests;

#[cfg(test)]
mod source_name_controls {
    use super::*;
    use crate::frontend::source::SourceFileId;

    fn source_name_paths(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join("oxid-owned-native-source-presentation-project");
        let path = root.join("src").join(name);
        (root, path)
    }

    fn checked_at(path: &str, text: &str) -> source::resource_fixtures::CheckedSource {
        source::SourceProgram::native_source_presentation_fixture(path, text)
    }

    fn emit_named(
        case: &source::resource_fixtures::CheckedSource,
        policy: NativeEntryPolicy,
        names: Option<&NativeSourceNames<'_>>,
        limits: Limits,
        accounting: &mut Accounting,
    ) -> Result<String, Box<Diagnostic>> {
        native_module_policy_accounted_with_source_names(
            &case.witness,
            Some(case.entry),
            &case.sources,
            plan::MAX_FUEL,
            limits,
            policy,
            accounting,
            names,
        )
    }

    #[test]
    fn owned_enum_native_source_names_preserve_transfer_counts_for_both_entry_modes() {
        let (logical_root, source_path) = source_name_paths("payload-雪.ox");
        let make = |path: &str| {
            let (original, raw, _, _) = execute::owned_enum_payload_tests::fixture();
            assert_eq!(original.files().len(), 1);
            let mut sources = SourceMap::new();
            sources.add(path.to_owned(), original.files()[0].text().to_owned());
            let witness = verified::verify_owned(raw, &sources).unwrap();
            source::resource_fixtures::CheckedSource {
                sources,
                witness,
                entry: hir::DefId(0),
            }
        };
        let case = make(source_path.to_str().unwrap());
        let reference = make("src/payload-雪.ox");
        let names = NativeSourceNames::project_relative(
            &case.sources,
            &logical_root,
            [(SourceFileId(0), source_path.as_path())],
        )
        .unwrap();
        for policy in [NativeEntryPolicy::Result, NativeEntryPolicy::Process] {
            let mut mapped = Accounting::default();
            let module =
                emit_named(&case, policy, Some(&names), Limits::DEFAULT, &mut mapped).unwrap();
            let mut expected = Accounting::default();
            let baseline =
                emit_named(&reference, policy, None, Limits::DEFAULT, &mut expected).unwrap();
            assert_eq!(module, baseline);
            assert_eq!(
                format!("{:?}", mapped.metrics),
                format!("{:?}", expected.metrics)
            );
            assert_eq!(mapped.metrics.transfer_cells, 2 + 5 + 9 + 12 + 10);
            assert_eq!(
                mapped.metrics.count_expansions,
                mapped.metrics.render_expansions
            );
            let mut denied = Accounting::default();
            let error = emit_named(
                &case,
                policy,
                Some(&names),
                Limits {
                    ir_bytes: module.len() - 1,
                    ..Limits::DEFAULT
                },
                &mut denied,
            )
            .unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains("LLVM bytes"));
            assert_eq!(denied.metrics.render_bytes, 0);
        }
    }

    #[test]
    fn owned_native_source_names_preserve_default_bytes_and_complete_accounting() {
        let (logical_root, source_path) = source_name_paths("雪.ox");
        let path = source_path.to_str().unwrap();
        for prefix in ["", "while false {}"] {
            let case = checked_at(path, &format!(
                "struct Number {{ value:i32 }} fn main()->i32 {{ let x=Number {{ value:7 }}; {prefix} return (x.value/3)%2; }}"
            ));
            let presented_case = checked_at("src/雪.ox", &format!(
                "struct Number {{ value:i32 }} fn main()->i32 {{ let x=Number {{ value:7 }}; {prefix} return (x.value/3)%2; }}"
            ));
            let relative = NativeSourceNames::project_relative(
                &case.sources,
                &logical_root,
                [(SourceFileId(0), source_path.as_path())],
            )
            .unwrap();
            for policy in [NativeEntryPolicy::Result, NativeEntryPolicy::Process] {
                let mut baseline = Accounting::default();
                let before = native_module_policy_accounted(
                    &case.witness,
                    Some(case.entry),
                    &case.sources,
                    plan::MAX_FUEL,
                    Limits::DEFAULT,
                    policy,
                    &mut baseline,
                )
                .unwrap();
                let mut mapped = Accounting::default();
                let after =
                    emit_named(&case, policy, Some(&relative), Limits::DEFAULT, &mut mapped)
                        .unwrap();
                let mut reference = Accounting::default();
                let expected_module = native_module_policy_accounted(
                    &presented_case.witness,
                    Some(presented_case.entry),
                    &presented_case.sources,
                    plan::MAX_FUEL,
                    Limits::DEFAULT,
                    policy,
                    &mut reference,
                )
                .unwrap();
                assert_eq!(after, expected_module);
                assert_eq!(
                    format!("{:?}", mapped.metrics),
                    format!("{:?}", reference.metrics)
                );
                assert!(after.len() < before.len());
                assert_eq!(mapped.metrics.count_bytes, after.len());
                assert_eq!(mapped.metrics.render_bytes, after.len());
                assert_eq!(
                    mapped.metrics.message_count_bytes,
                    mapped.metrics.message_render_bytes
                );
                assert!(mapped.metrics.message_bytes < baseline.metrics.message_bytes);
                assert_eq!(mapped.metrics.row_sizes, baseline.metrics.row_sizes);
                assert_eq!(
                    mapped.metrics.occurrence_bytes,
                    baseline.metrics.occurrence_bytes
                );
                assert_eq!(mapped.metrics.source_bytes, baseline.metrics.source_bytes);
                assert_eq!(
                    mapped.metrics.source_scalars,
                    baseline.metrics.source_scalars
                );
                assert_eq!(
                    mapped.metrics.allocation_attempts,
                    baseline.metrics.allocation_attempts
                );
                assert_eq!(mapped.metrics.metadata_peak, baseline.metrics.metadata_peak);

                let plan = ExecutionPlan::build(&case.witness).unwrap();
                let guarded = policy == NativeEntryPolicy::Process || !prefix.is_empty();
                let expected = Diagnostics::new_policy_accounted(
                    &plan,
                    case.entry,
                    &case.sources,
                    guarded,
                    policy,
                    MAX_DIAGNOSTIC_BYTES,
                    Limits::DEFAULT.metadata_bytes,
                    &mut Accounting::default(),
                )
                .unwrap();
                let actual = Diagnostics::new_policy_accounted_with_source_names(
                    &plan,
                    case.entry,
                    &case.sources,
                    guarded,
                    policy,
                    MAX_DIAGNOSTIC_BYTES,
                    Limits::DEFAULT.metadata_bytes,
                    &mut Accounting::default(),
                    Some(&relative),
                )
                .unwrap();
                assert!(!expected.messages.is_empty());
                assert_eq!(
                    actual.messages,
                    expected
                        .messages
                        .iter()
                        .map(|text| text.replace(path, "src/雪.ox"))
                        .collect::<Vec<_>>()
                );
                assert_eq!(actual.ids, expected.ids);
            }
        }
    }

    #[test]
    fn owned_native_source_names_count_mapped_diagnostics_before_reserving_text() {
        let (logical_root, source_path) = source_name_paths("main.ox");
        let path = source_path.to_str().unwrap();
        let case = checked_at(path,
            "struct Number { value:i32 } fn main()->i32 { let x=Number { value:7 }; while false {} return x.value/0; }"
        );
        let names = NativeSourceNames::project_relative(
            &case.sources,
            &logical_root,
            [(SourceFileId(0), source_path.as_path())],
        )
        .unwrap();
        for policy in [NativeEntryPolicy::Result, NativeEntryPolicy::Process] {
            let mut accounting = Accounting::default();
            let module = emit_named(
                &case,
                policy,
                Some(&names),
                Limits::DEFAULT,
                &mut accounting,
            )
            .unwrap();
            let diagnostic_bytes = accounting.metrics.message_bytes;
            for less in [0, 1] {
                for (limits, marker) in [
                    (
                        Limits {
                            diagnostic_bytes: diagnostic_bytes - less,
                            ..Limits::DEFAULT
                        },
                        "diagnostic bytes",
                    ),
                    (
                        Limits {
                            ir_bytes: module.len() - less,
                            ..Limits::DEFAULT
                        },
                        "LLVM bytes",
                    ),
                ] {
                    let mut attempt = Accounting::default();
                    let result = emit_named(&case, policy, Some(&names), limits, &mut attempt);
                    if less == 0 {
                        assert_eq!(result.unwrap(), module);
                    } else {
                        let error = result.unwrap_err();
                        assert_eq!(error.code, "E0700");
                        assert!(error.message.contains(marker), "{}", error.message);
                        assert_eq!(attempt.metrics.render_bytes, 0);
                        if marker == "diagnostic bytes" {
                            assert_eq!(attempt.metrics.message_render_bytes, 0);
                        }
                    }
                }
            }
            let error = emit_named(
                &case,
                policy,
                None,
                Limits {
                    diagnostic_bytes,
                    ..Limits::DEFAULT
                },
                &mut Accounting::default(),
            )
            .unwrap_err();
            assert!(error.message.contains("diagnostic bytes"));
        }
    }

    #[test]
    fn owned_native_source_names_reject_another_map_before_native_allocation() {
        let (logical_root, source_path) = source_name_paths("main.ox");
        let path = source_path.to_str().unwrap();
        let text = "struct Number { value:i32 } fn main()->i32 { let x=Number { value:7 }; return x.value; }";
        let case = checked_at(path, text);
        let other = checked_at(path, text);
        let names = NativeSourceNames::project_relative(
            &other.sources,
            &logical_root,
            [(SourceFileId(0), source_path.as_path())],
        )
        .unwrap();
        for policy in [NativeEntryPolicy::Result, NativeEntryPolicy::Process] {
            let mut accounting = Accounting::default();
            let error = emit_named(
                &case,
                policy,
                Some(&names),
                Limits::DEFAULT,
                &mut accounting,
            )
            .unwrap_err();
            assert_eq!(error.code, "E0500");
            assert_eq!(accounting.metrics.plan_bytes, 0);
            assert_eq!(accounting.metrics.allocation_attempts, 0);
            assert_eq!(accounting.metrics.count_bytes, 0);
            assert_eq!(accounting.metrics.render_bytes, 0);
        }
    }
}

#[cfg(test)]
mod u8_inventory_controls {
    use super::*;

    #[test]
    fn owned_u8_native_inventory_exact_text_and_metadata_endpoints() {
        for guarded in [false, true] {
            let (sources, raw, _) = super::super::u8_tests::raw(255, guarded);
            let witness = source::u8_tests::verify_source_raw(raw, &sources).unwrap();
            let mut accounting = Accounting::default();
            let ir = native_module_accounted(
                &witness,
                Some(hir::DefId(0)),
                &sources,
                12,
                Limits::DEFAULT,
                &mut accounting,
            )
            .unwrap();
            let metrics = &accounting.metrics;
            assert_eq!(metrics.count_bytes, ir.len());
            assert_eq!(metrics.render_bytes, ir.len());
            assert_eq!(
                metrics.count_ordinary_visits,
                metrics.render_ordinary_visits
            );
            assert_eq!(metrics.message_count_bytes, metrics.message_render_bytes);
            assert!(metrics.unique >= 1);
            assert!(
                FailureKind::ByteRange
                    .diagnostic(witness.functions()[0].span, &sources)
                    .message
                    .len()
                    <= 64
            );
            for less in [0, 1] {
                let limits = Limits {
                    ir_bytes: ir.len() - less,
                    ..Limits::DEFAULT
                };
                assert_eq!(
                    native_module_limits(&witness, Some(hir::DefId(0)), &sources, 12, limits)
                        .is_ok(),
                    less == 0
                );
                let limits = Limits {
                    metadata_bytes: metrics.metadata_peak - less,
                    ..Limits::DEFAULT
                };
                assert_eq!(
                    native_module_limits(&witness, Some(hir::DefId(0)), &sources, 12, limits)
                        .is_ok(),
                    less == 0
                );
            }
            println!("RFC0030 owned native guarded={guarded} {metrics:?}");
        }
    }

    #[test]
    fn owned_u8_native_entry_rejects_without_plan_or_reservation() {
        let (sources, raw) = source::u8_tests::raw_source(
            "struct R {} fn main()->u8{let x=255;return x.to_u8_checked();}",
        );
        let witness = source::u8_tests::verify_source_raw(raw, &sources).unwrap();
        let mut accounting = Accounting::default();
        let error = native_module_accounted(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            0,
            Limits::DEFAULT,
            &mut accounting,
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
        assert_eq!(accounting.metrics.plan_bytes, 0);
        assert_eq!(accounting.metrics.allocation_attempts, 0);
        assert_eq!(accounting.metrics.count_bytes, 0);
    }
}

#[cfg(test)]
#[test]
fn owned_u8_native_containing_transport_layouts() {
    use std::mem::{align_of, size_of};
    macro_rules! report {($($t:ty),+)=>{$(println!("RFC0030 owned native {} size={} align={}",stringify!($t),size_of::<$t>(),align_of::<$t>());)+}}
    report!(
        FailureKind,
        Continuation,
        DiagnosticKey,
        DiagnosticOccurrence,
        Diagnostics,
        Bound,
        NativeMetrics,
        Accounting,
        Emission
    );
}

// Complete named-role inventory for the RFC0030 scalar emission arms. These
// are conservative sums of construction, return and caller roles, rather than
// a claim about Rust's machine stack or allocator capacity. The emitted text
// streams directly into the already-admitted Emission; no IR fragment String
// is constructed. Operand names and the final result are the only new arm's
// owned strings, with the same lifecycle as CheckedNegateI32.
/// Compiler-generated callsite backing measured with this toolchain's exact
/// conversion/predecessor format templates in owned-format-callsite.mir. The
/// capture tuple uses thin references. Each rt::Argument is two pointer-sized
/// words (16 bytes, alignment8 on the qualified host), separately counted for
/// constructor return and array storage. Template and array borrows are each
/// one pointer; template bytes themselves are static compiled data. This is a
/// role model, not access to the private std type or a stdlib-frame estimate.
#[allow(dead_code)]
struct FormatCallsiteBacking<const N: usize> {
    captures: [usize; N],
    argument_returns: [[usize; 2]; N],
    arguments: [[usize; 2]; N],
    template_borrow: usize,
    argument_array_borrow: usize,
    forwarded_arguments: std::fmt::Arguments<'static>,
}

#[allow(dead_code)]
struct U8EmissionCarriers {
    storage: &'static NativeFunctionStorage<'static, 'static>,
    plan: &'static ExecutionPlan<'static>,
    function: &'static RawOwnedFunction,
    instruction: &'static OwnedStatement,
    assignment: &'static Assign,
    name: &'static str,
    emitter: &'static mut Emission,
    diagnostics: &'static Diagnostics,
    function_id: hir::DefId,
    destination_type: hir::Ty,
    // Both comparison operands are included even for unary conversion arms.
    operands: [Operand; 2],
    origin: Span,
    continuation: Continuation,
    suffix_option: Option<&'static str>,
    suffix: &'static str,
    unsigned: bool,
    operand_type: &'static str,
    predicate: &'static str,
    // Caller name; name construction and load-slot/load-operand return plus
    // caller storage for each operand; final result construction/caller; final
    // store name and pointer. Distinct headers are not assumed to be elided.
    strings: [String; 13],
    diagnostic_key: DiagnosticKey,
    diagnostic_search: Result<usize, usize>,
    diagnostic_index: usize,
    diagnostic_id: usize,
    diagnostic_length: usize,
    diagnostic_return: (usize, usize),
    diagnostic_pattern: (usize, usize),
    failure_helper: &'static str,
    // Per format/write call: complete public Arguments, displayed borrowed
    // inputs and Result. Five displayed inputs cover the unchanged comparison formatter;
    // the conversion maximum is three. The full sequential call counts are
    // summed, including two loads, failure emission and final store.
    formatting: [std::fmt::Arguments<'static>; 14],
    callsite_backing: [FormatCallsiteBacking<5>; 14],
    formatting_results: [std::fmt::Result; 14],
}
// A usize has at most BITS decimal digits. All generated scalar names have
// f{function}_b{block}_i{instruction}; the longest new owned suffix is _operand,
// with one leading %. IDs are verifier-admitted, never source identifier text.
const U8_NAME_BYTES: usize = 3 * usize::BITS as usize + 5 + 8 + 1;
const U8_EMISSION_BYTES: usize = size_of::<U8EmissionCarriers>() + 13 * U8_NAME_BYTES;
// Numerical containment check only, not a claim that unlisted predecessor
// roles are spare. The conversion branches are dominated by predecessor checked
// arithmetic as documented and checked in the role-comparison test below.
const _: () = assert!(
    U8_EMISSION_BYTES
        + size_of::<ScalarLeaves<'static>>()
        + plan::native_storage::FIXED_CARRIER_ALLOWANCE
        <= EMITTER_TRANSIENT_BYTES
);

#[test]
fn owned_u8_native_operation_carriers_fit_existing_transient_envelope() {
    let inherited = plan::native_storage::FIXED_CARRIER_ALLOWANCE;
    let leaves = size_of::<ScalarLeaves<'static>>();
    assert_eq!(EMITTER_TRANSIENT_BYTES, 32_768);
    assert!(U8_EMISSION_BYTES + leaves + inherited <= EMITTER_TRANSIENT_BYTES);
    let name = format!("f{}_b{}_i{}", usize::MAX, usize::MAX, usize::MAX);
    for value in [
        format!("%{name}_operand"),
        format!("%{name}_value"),
        format!("{name}_store"),
    ] {
        assert!(value.len() <= U8_NAME_BYTES);
    }
    println!("RFC0030 owned native scalar_arm_headers={} name_payload={} arm_total={} inherited_native_plan={} scalar_leaves={} conservative_subdivision={} unchanged_emitter_cap={}",
        size_of::<U8EmissionCarriers>(), 13 * U8_NAME_BYTES, U8_EMISSION_BYTES,
        inherited, leaves, U8_EMISSION_BYTES + inherited + leaves, EMITTER_TRANSIENT_BYTES);
}

// Relative proof against the already-present CheckedI32 emitter, independent
// of treating any part of EMITTER_TRANSIENT_BYTES as spare: both paths retain
// the same outer emit-function/statement/scalar callers, output/diagnostic tables,
// common result and store_slot roles. CheckedI32 retains two Operand bindings,
// an ArithmeticOp and an operator Span; conversion retains one Operand and its
// name Span (source_expr is not bound by the emission match). Both retain one
// static continuation suffix. The conversion's one load uses the same helpers
// as each of CheckedI32's two loads. The one longer _operand name is bounded by
// the sum of CheckedI32's _left and _right names. Both call emit_failure once
// on the relevant overflow/range branch, using the same key/search/return roles.
// Conversion's largest write uses three captures; CheckedI32 uses four.
// MIR-derived capture tuples, private Argument constructor returns/arrays,
// template and argument-array borrows are explicitly included in both models.
// Narrowing performs fewer format/write calls; widening omits the suffix/failure.
// Different instruction text streams into the separately exact-counted output.
#[allow(dead_code)]
struct PredecessorCheckedArmRoles {
    operands: [Operand; 2],
    operator: hir::ArithmeticOp,
    origin: Span,
    suffix_option: Option<&'static str>,
    suffix: &'static str,
    operand_names: [String; 2],
    formatted: std::fmt::Arguments<'static>,
    backing: FormatCallsiteBacking<4>,
    result: std::fmt::Result,
}
#[allow(dead_code)]
struct CheckedByteArmRoles {
    operand: Operand,
    origin: Span,
    suffix_option: Option<&'static str>,
    suffix: &'static str,
    operand_name: String,
    formatted: std::fmt::Arguments<'static>,
    backing: FormatCallsiteBacking<3>,
    result: std::fmt::Result,
}
#[allow(dead_code)]
struct PredecessorComparisonRoles {
    operation: hir::ComparisonOp,
    operand_type: &'static str,
    left: String,
    right: String,
    predicate: &'static str,
}
#[allow(dead_code)]
struct ByteComparisonRoles {
    operation: hir::ComparisonOp,
    unsigned: bool,
    operand_type: &'static str,
    left: String,
    right: String,
    predicate: &'static str,
}
#[test]
fn owned_u8_native_roles_are_dominated_by_predecessor_operations() {
    // Exact mirrored MIR arities: narrow [3,3], widen [2], predecessor
    // arithmetic [2,3,4,2,2,4,1]. Repeated named placeholders are deduplicated;
    // the old and new comparison template remains identical with five captures.
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            (size_of::<[usize; 2]>(), std::mem::align_of::<[usize; 2]>()),
            (16, 8)
        );
        assert_eq!(size_of::<std::fmt::Arguments<'static>>(), 16);
        assert_eq!(size_of::<FormatCallsiteBacking<3>>(), 152);
        assert_eq!(size_of::<FormatCallsiteBacking<4>>(), 192);
    }
    assert!(size_of::<CheckedByteArmRoles>() <= size_of::<PredecessorCheckedArmRoles>());
    // The sole retained comparison addition occupies padding in the measured
    // complete role carrier; formatting and both operand helpers are unchanged.
    assert_eq!(
        size_of::<ByteComparisonRoles>(),
        size_of::<PredecessorComparisonRoles>()
    );
    for name in [
        "f0_b0_i0".to_string(),
        format!("f{}_b{}_i{}", usize::MAX, usize::MAX, usize::MAX),
    ] {
        assert!(
            format!("%{name}_operand").len()
                <= format!("%{name}_left").len() + format!("%{name}_right").len()
        );
    }
    println!("RFC0030 native relative carriers checked_byte={} predecessor_checked={} byte_comparison={} predecessor_comparison={}; same outer callers and failure/store helpers", size_of::<CheckedByteArmRoles>(),size_of::<PredecessorCheckedArmRoles>(),size_of::<ByteComparisonRoles>(),size_of::<PredecessorComparisonRoles>());
}

#[cfg(test)]
#[path = "byte_storage_native_tests.rs"]
mod byte_storage_tests;

#[cfg(test)]
#[test]
fn owned_enum_native_nested_transfer_checks_before_stores_and_counts_branches() {
    let (sources, raw, _, _) = execute::owned_enum_payload_tests::fixture();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let observed = run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        NativeControl::default(),
    );
    let module = observed.result.unwrap();
    // Two record cells, then constructor tag + four active leaf operations;
    // outer constructor tag + four inner case rows + four leaf operations;
    // whole copy has eight rows + four leaves; discard has eight rows + two
    // validations. No inactive alternative or padding contributes a leaf.
    assert_eq!(observed.metrics.transfer_cells, 2 + 5 + 9 + 12 + 10);
    assert_eq!(
        observed.metrics.count_expansions,
        observed.metrics.transfer_cells + observed.metrics.message_bytes
    );
    assert_eq!(
        observed.metrics.count_expansions,
        observed.metrics.render_expansions
    );
    assert_eq!(observed.metrics.count_bytes, module.len());
    assert_eq!(observed.metrics.render_bytes, module.len());
    assert_eq!(
        observed.metrics.count_ordinary_visits,
        observed.metrics.render_ordinary_visits
    );
    let name = "f0_b0_i9";
    let nested_tag = module
        .find(&format!("%{name}_nested1_tag = load i32"))
        .unwrap();
    let valid_bool = module
        .find(&format!("{name}_nested1_variant0_validate0_ok:"))
        .unwrap();
    let copied_payload = module
        .find(&format!("%{name}_nested1_variant0_copy_field0 = load i1"))
        .unwrap();
    let stored_outer_tag = module
        .find(&format!("{name}_nested1_enum_ok:\n  store i32 0, ptr %o3"))
        .unwrap();
    assert!(
        nested_tag < valid_bool && valid_bool < copied_payload && copied_payload < stored_outer_tag
    );
    assert!(!module.contains("memcpy"));
}

#[test]
fn owned_enum_native_shared_branch_counts_and_exact_ir_admission() {
    let (sources, mut raw, _, _) = execute::owned_enum_payload_tests::fixture();
    // Both outer alternatives share one canonical inner enum. Declaration
    // layout counts that child once per maximum path, but native branch text
    // must include the child switch and leaves in both alternatives.
    raw.enums[1].variants[1].payload = Some(ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(
        EnumId(0),
    ))));
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))),
        Ok(Scalar::I32(42))
    );
    let mut accounting = Accounting::default();
    let module = native_module_policy_accounted(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        NativeEntryPolicy::Result,
        &mut accounting,
    )
    .unwrap();
    // Independent hand count: record construction2, inner construction5,
    // outer construction9, whole copy(outer4 + two inner4 + two leaf4)=20,
    // discard(outer4 + two inner4 + two leaf2)=16. Seven emitted switches.
    assert_eq!(accounting.metrics.transfer_cells, 2 + 5 + 9 + 20 + 16);
    assert_eq!(module.matches("switch i32").count(), 7);
    assert_eq!(
        accounting.metrics.count_expansions,
        accounting.metrics.render_expansions
    );
    assert_eq!(
        accounting.metrics.count_ordinary_visits,
        accounting.metrics.render_ordinary_visits
    );
    assert_eq!(accounting.metrics.count_bytes, module.len());
    assert_eq!(accounting.metrics.render_bytes, module.len());
    for less in [0, 1] {
        let mut accounting = Accounting::default();
        let result = native_module_policy_accounted(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            plan::MAX_FUEL,
            Limits {
                ir_bytes: module.len() - less,
                ..Limits::DEFAULT
            },
            NativeEntryPolicy::Result,
            &mut accounting,
        );
        if less == 0 {
            assert_eq!(result.unwrap(), module);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains("LLVM bytes"));
            assert_eq!(accounting.metrics.render_bytes, 0);
        }
    }
}
