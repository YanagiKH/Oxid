//! Private owned LLVM consumer. All metadata is bound to one immutable witness
//! by ExecutionPlan; the scalar source route does not enter this module.
use super::{plan::ExecutionPlan, verified::VerifiedOwnedProgram, *};
use std::fmt::Write;

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
#[derive(Clone, Copy)]
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
    let limits = limits.bounded();
    let bounds = admit(&plan, limits)?;
    let guarded = bounds.iter().any(|b| b.cyclic);
    let diagnostics = Diagnostics::new(&plan, id, sources, guarded, limits.diagnostic_bytes)?;
    let mut count = Emission::count(limits.ir_bytes);
    emit(
        &plan,
        id,
        &diagnostics,
        guarded,
        fuel.min(plan::MAX_FUEL),
        &mut count,
    );
    if count.exceeded {
        return Err(reject(
            format!(
                "native owned LLVM bytes limit exceeded ({})",
                limits.ir_bytes
            ),
            Some(root.span),
        ));
    }
    let mut text = String::new();
    text.try_reserve_exact(count.len)
        .map_err(|_| reject("native owned LLVM allocation", None))?;
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
    let functions = plan.witness().functions();
    let origin = functions[0].span; // Entry validation proves a nonempty program.
    limit(functions.len(), limits.functions, "function count", origin)?;
    let mut callers = vec![Vec::new(); functions.len()];
    let mut remaining = vec![0usize; functions.len()];
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
                callers[f.calls[call.0].target.0].push(f.id.0);
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
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        let f = &functions[i];
        let u = plan.function(f.id).usage();
        // Determine unknown cost before adding any placeholders: even a late
        // cyclic callee makes the entire static sum nonbinding. Resource and
        // known acyclic arithmetic stay checked, while unknown sums saturate.
        let cyclic = has_cycle(f)
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
fn has_cycle(f: &RawOwnedFunction) -> bool {
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
    }
    visited != f.blocks.len()
}

struct Emission {
    len: usize,
    text: Option<String>,
    maximum: usize,
    exceeded: bool,
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
}
impl Write for LimitedCount {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.len = self
            .len
            .checked_add(text.len())
            .filter(|&n| n <= self.maximum)
            .ok_or(std::fmt::Error)?;
        Ok(())
    }
}
type DiagnosticKey = (bool, usize, usize, usize);
struct Diagnostics {
    messages: Vec<String>,
    ids: std::collections::BTreeMap<DiagnosticKey, usize>,
}
impl Diagnostics {
    fn new(
        plan: &ExecutionPlan<'_>,
        entry: hir::DefId,
        sources: &SourceMap,
        guarded: bool,
        maximum: usize,
    ) -> Result<Self, Box<Diagnostic>> {
        let mut result = Self {
            messages: Vec::new(),
            ids: std::collections::BTreeMap::new(),
        };
        let mut count = LimitedCount { len: 0, maximum };
        let mut add = |overflow: bool, span: Span| -> Result<(), Box<Diagnostic>> {
            let key = (overflow, span.file.0, span.start, span.end);
            if result.ids.contains_key(&key) {
                return Ok(());
            }
            let diagnostic = if overflow {
                RunFailure::Overflow(span)
            } else {
                RunFailure::Fuel(span)
            }
            .diagnostic(sources);
            diagnostic.write_human(sources, &mut count).map_err(|_| {
                reject(
                    format!("native owned diagnostic bytes limit exceeded ({maximum})"),
                    Some(span),
                )
            })?;
            result.ids.insert(key, result.messages.len());
            result.messages.push(diagnostic.render_human(sources));
            Ok(())
        };
        if guarded {
            add(false, plan.witness().functions()[entry.0].span)?;
        }
        for f in plan.witness().functions() {
            for b in &f.blocks {
                if guarded {
                    if let Some(m) = &b.merge {
                        add(false, m.span)?;
                    }
                }
                for statement in &b.statements {
                    if guarded {
                        add(false, plan::instruction_span(statement))?;
                    }
                    if let OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI32 { operator_span, .. },
                        ..
                    })) = &statement.kind
                    {
                        add(true, *operator_span)?;
                    }
                }
                if guarded {
                    add(
                        false,
                        b.terminator.as_ref().expect("verified terminator").span,
                    )?;
                }
            }
        }
        Ok(result)
    }
    fn get(&self, overflow: bool, span: Span) -> (usize, usize) {
        let id = self.ids[&(overflow, span.file.0, span.start, span.end)];
        (id, self.messages[id].len())
    }
}
fn emit_failure(out: &mut Emission, diagnostics: &Diagnostics, overflow: bool, span: Span) {
    let (id, len) = diagnostics.get(overflow, span);
    writeln!(
        out,
        "  call void @__oxid_overflow(ptr @__oxid_owned_error_{id}, i64 {len})\n  unreachable"
    )
    .unwrap();
}
fn emit_guard(out: &mut Emission, diagnostics: &Diagnostics, name: &str, cost: usize, span: Span) {
    writeln!(out, "  %{name}_remaining = load i64, ptr %fuel\n  %{name}_exhausted = icmp ult i64 %{name}_remaining, {cost}\n  br i1 %{name}_exhausted, label %{name}_error, label %{name}_ok\n{name}_error:").unwrap();
    emit_failure(out, diagnostics, false, span);
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
            emit_failure(out, diagnostics, true, operator_span);
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
            writeln!(
                out,
                "  {prefix}call {} @__oxid_owned_fn_{}({})",
                result_ty(callee.result),
                descriptor.target.0,
                args.join(", ")
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
