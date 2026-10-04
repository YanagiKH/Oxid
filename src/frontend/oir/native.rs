//! Admission and LLVM lowering consume only the verifier's immutable witness.
//! Admission is intentionally stricter than checking/reference execution.
use super::*;
use std::fmt::Write;

const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMS: usize = 64;
const MAX_FUNCTION_LOCALS: usize = 256;
const MAX_NATIVE_LOCALS: usize = 8_192;
const MAX_NATIVE_BLOCKS: usize = 4_096;
const MAX_DEPTH: usize = 32;
const MAX_COST: usize = 100_000;

fn reject(message: impl Into<String>, span: Option<Span>) -> Box<Diagnostic> {
    Diagnostic::new("E0700", "native-admission", message, span)
}
fn limit(value: usize, maximum: usize, name: &str, span: Span) -> Result<(), Box<Diagnostic>> {
    if value > maximum {
        Err(reject(
            format!("native preview {name} limit exceeded ({maximum})"),
            Some(span),
        ))
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Bound {
    cost: usize,
    cyclic: bool,
    depth: usize,
    slots: usize,
}

impl VerifiedProgram {
    pub(in crate::frontend) fn native_module(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
    ) -> Result<String, Box<Diagnostic>> {
        self.native_module_fuel(entry, sources, execute::MAX_FUEL)
    }

    #[cfg(test)]
    pub(super) fn native_module_with_fuel(
        &self,
        entry: hir::DefId,
        sources: &SourceMap,
        fuel: usize,
    ) -> Result<String, Box<Diagnostic>> {
        self.native_module_fuel(Some(entry), sources, fuel.min(execute::MAX_FUEL))
    }

    fn native_module_fuel(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
        fuel: usize,
    ) -> Result<String, Box<Diagnostic>> {
        self.native_module_limits(
            entry,
            sources,
            fuel,
            MAX_GUARDED_DIAGNOSTIC_BYTES,
            MAX_GUARDED_IR_BYTES,
        )
    }

    fn native_module_limits(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
        fuel: usize,
        diagnostic_limit: usize,
        ir_limit: usize,
    ) -> Result<String, Box<Diagnostic>> {
        let id = entry.ok_or_else(|| {
            reject(
                "native compile requires a declared zero-argument main",
                None,
            )
        })?;
        let root = self
            .program
            .functions
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
        if root.param_count != 0 {
            return Err(reject(
                "native main must have no parameters",
                Some(root.span),
            ));
        }
        let bounds = self.admit()?;
        let guarded = bounds.iter().any(|bound| bound.cyclic);
        let diagnostics = guarded
            .then(|| GuardedDiagnostics::new(&self.program, id, sources, diagnostic_limit))
            .transpose()?;
        // Count the exact emitted UTF-8 bytes without allocating LLVM text.
        let mut count = Emission::default();
        emit(
            &self.program,
            id,
            sources,
            diagnostics.as_ref(),
            fuel,
            &mut count,
        );
        if guarded {
            limit(count.len, ir_limit, "guarded LLVM bytes", root.span)?;
        }
        let mut output = Emission {
            len: 0,
            text: Some(String::with_capacity(count.len)),
        };
        emit(
            &self.program,
            id,
            sources,
            diagnostics.as_ref(),
            fuel,
            &mut output,
        );
        debug_assert_eq!(output.len, count.len);
        Ok(output.text.expect("render pass"))
    }

    fn admit(&self) -> Result<Vec<Bound>, Box<Diagnostic>> {
        let functions = &self.program.functions;
        if functions.is_empty() {
            return Ok(Vec::new());
        }
        let origin = functions[0].span;
        limit(functions.len(), MAX_FUNCTIONS, "function count", origin)?;
        let mut callers = vec![Vec::new(); functions.len()];
        let mut remaining = vec![0usize; functions.len()];
        let (mut locals, mut blocks) = (0, 0);
        for f in functions {
            limit(f.param_count, MAX_PARAMS, "parameter count", f.span)?;
            limit(
                f.slot_count(),
                MAX_FUNCTION_LOCALS,
                "locals per function",
                f.span,
            )?;
            locals += f.slot_count();
            blocks += f.blocks.len();
            for b in &f.blocks {
                // The only entry operation is the independently verified,
                // fixed two-input bool merge; no generic phi types are admitted.
                // Closed native allowlist: future reference operations stay unsupported.
                for statement in &b.statements {
                    let a = match statement {
                        Statement::Assign(a) => a,
                        Statement::Initialize { .. } | Statement::Store { .. } => continue,
                    };
                    #[allow(unreachable_patterns)]
                    match a.value {
                        Rvalue::Bool(_)
                        | Rvalue::I32(_)
                        | Rvalue::Unit
                        | Rvalue::Copy(_)
                        | Rvalue::Load(_)
                        | Rvalue::NotBool { .. }
                        | Rvalue::CheckedI32 { .. }
                        | Rvalue::CompareScalar { .. } => {}
                        _ => {
                            return Err(reject(
                                "native preview does not support this OIR operation",
                                Some(a.span),
                            ))
                        }
                    }
                }
                if let TerminatorKind::Call { target, .. } =
                    &b.terminator.as_ref().expect("verified terminator").kind
                {
                    remaining[f.id.0] += 1;
                    callers[target.0].push(f.id.0);
                }
            }
        }
        limit(locals, MAX_NATIVE_LOCALS, "aggregate locals", origin)?;
        limit(blocks, MAX_NATIVE_BLOCKS, "aggregate blocks", origin)?;
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
            let mut bound = Bound {
                cost: f.slot_count(),
                cyclic: has_cycle(f),
                depth: 1,
                slots: f.slot_count(),
            };
            // Summing blocks bounds execution only when this CFG and all
            // transitive callees are acyclic. Unknown cyclic cost propagates
            // separately; it is never mistaken for this static sum.
            for b in &f.blocks {
                bound.cost = bound
                    .cost
                    .saturating_add(b.statements.len() + 1 + usize::from(b.merge.is_some()));
                if let TerminatorKind::Call { target, args, .. } =
                    &b.terminator.as_ref().expect("verified terminator").kind
                {
                    let child = bounds[target.0];
                    bound.cyclic |= child.cyclic;
                    bound.cost = bound
                        .cost
                        .saturating_add(args.len())
                        .saturating_add(child.cost);
                    bound.depth = bound.depth.max(1 + child.depth);
                    bound.slots = bound.slots.max(f.slot_count() + child.slots);
                }
            }
            // Preserve the old inclusive static bound for every function whose
            // transitive CFG is acyclic. Cyclic costs require a shared runtime
            // guard, while depth/live storage remain statically bounded.
            if !bound.cyclic {
                limit(
                    bound.cost.saturating_add(1),
                    MAX_COST,
                    "reference fuel upper bound",
                    f.span,
                )?;
            }
            limit(bound.depth, MAX_DEPTH, "call depth", f.span)?;
            limit(bound.slots, MAX_NATIVE_LOCALS, "live local slots", f.span)?;
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
        Ok(bounds)
    }
}
// Guarded modules retain all old OIR/source/native dimensions, and additionally
// bound their expanded representation. Loop-free modules keep old admission.
const MAX_GUARDED_DIAGNOSTIC_BYTES: usize = 16 * 1024 * 1024;
const MAX_GUARDED_IR_BYTES: usize = 64 * 1024 * 1024;

fn has_cycle(function: &Function) -> bool {
    let mut incoming = vec![0usize; function.blocks.len()];
    for block in &function.blocks {
        for target in successors(&block.terminator.as_ref().expect("verified terminator").kind) {
            incoming[target.0] += 1;
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(id, &n)| (n == 0).then_some(id))
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        for target in successors(
            &function.blocks[id]
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
    visited != function.blocks.len()
}
fn successors(kind: &TerminatorKind) -> impl Iterator<Item = BlockId> {
    let targets = match kind {
        TerminatorKind::Return(_) => [None, None],
        TerminatorKind::Goto { target } => [Some(*target), None],
        TerminatorKind::Call { continuation, .. } => [Some(*continuation), None],
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => [Some(*then_block), Some(*else_block)],
    };
    targets.into_iter().flatten()
}

#[derive(Default)]
struct Emission {
    len: usize,
    text: Option<String>,
}
impl Emission {
    fn push_str(&mut self, text: &str) {
        // Saturation cannot admit an overflow: the guarded count is compared
        // with 64MiB before allocation, and unguarded inputs retain old bounds.
        self.len = self.len.saturating_add(text.len());
        if let Some(buffer) = &mut self.text {
            buffer.push_str(text);
        }
    }
}
impl Write for Emission {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.push_str(text);
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FailureKind {
    Fuel,
    Overflow,
    DivisionByZero,
}
impl FailureKind {
    fn diagnostic(self, span: Span, sources: &SourceMap) -> Box<Diagnostic> {
        match self {
            Self::Fuel => RunFailure::Fuel(span),
            Self::Overflow => RunFailure::Overflow(span),
            Self::DivisionByZero => RunFailure::DivisionByZero(span),
        }
        .diagnostic(sources)
    }
    fn arithmetic_symbol(self) -> &'static str {
        match self {
            Self::Overflow => "__oxid_error",
            Self::DivisionByZero => "__oxid_division_error",
            Self::Fuel => unreachable!("arithmetic failure kind"),
        }
    }
}
fn arithmetic_failures(op: hir::ArithmeticOp) -> &'static [FailureKind] {
    match op {
        hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
            &[FailureKind::Overflow, FailureKind::DivisionByZero]
        }
        hir::ArithmeticOp::Add | hir::ArithmeticOp::Subtract | hir::ArithmeticOp::Multiply => {
            &[FailureKind::Overflow]
        }
    }
}
type DiagnosticKey = (FailureKind, usize, usize, usize);
struct GuardedDiagnostics {
    messages: Vec<String>,
    ids: std::collections::BTreeMap<DiagnosticKey, usize>,
}
impl GuardedDiagnostics {
    fn new(
        program: &Program,
        entry: hir::DefId,
        sources: &SourceMap,
        maximum: usize,
    ) -> Result<Self, Box<Diagnostic>> {
        let mut result = Self {
            messages: Vec::new(),
            ids: std::collections::BTreeMap::new(),
        };
        let mut count = LimitedCount { len: 0, maximum };
        let mut add = |kind: FailureKind, span: Span| -> Result<(), Box<Diagnostic>> {
            let key = (kind, span.file.0, span.start, span.end);
            if result.ids.contains_key(&key) {
                return Ok(());
            }
            let diagnostic = kind.diagnostic(span, sources);
            // The renderer streams escaped paths while counting: no oversize
            // intermediate diagnostic is allocated before this preflight.
            diagnostic.write_human(sources, &mut count).map_err(|_| {
                reject(
                    format!("native preview guarded diagnostic bytes limit exceeded ({maximum})"),
                    Some(span),
                )
            })?;
            result.ids.insert(key, result.messages.len());
            result.messages.push(diagnostic.render_human(sources));
            Ok(())
        };
        add(FailureKind::Fuel, program.functions[entry.0].span)?;
        for function in &program.functions {
            for block in &function.blocks {
                if let Some(merge) = &block.merge {
                    add(FailureKind::Fuel, merge.span)?;
                }
                for statement in &block.statements {
                    add(FailureKind::Fuel, statement.span())?;
                    if let Some(Assign {
                        value:
                            Rvalue::CheckedI32 {
                                op, operator_span, ..
                            },
                        ..
                    }) = statement.as_assignment()
                    {
                        for &kind in arithmetic_failures(*op) {
                            add(kind, *operator_span)?;
                        }
                    }
                }
                add(
                    FailureKind::Fuel,
                    block.terminator.as_ref().expect("verified terminator").span,
                )?;
            }
        }
        Ok(result)
    }
    fn get(&self, kind: FailureKind, span: Span) -> (usize, &str) {
        let id = self.ids[&(kind, span.file.0, span.start, span.end)];
        (id, &self.messages[id])
    }
}
fn emit_guard(
    out: &mut Emission,
    diagnostics: &GuardedDiagnostics,
    name: &str,
    cost: usize,
    span: Span,
) {
    let (id, message) = diagnostics.get(FailureKind::Fuel, span);
    writeln!(out, "  %{name}_remaining = load i64, ptr %fuel").unwrap();
    writeln!(
        out,
        "  %{name}_exhausted = icmp ult i64 %{name}_remaining, {cost}"
    )
    .unwrap();
    writeln!(
        out,
        "  br i1 %{name}_exhausted, label %{name}_error, label %{name}_ok"
    )
    .unwrap();
    writeln!(out, "{name}_error:\n  call void @__oxid_overflow(ptr @__oxid_guard_error_{id}, i64 {})\n  unreachable", message.len()).unwrap();
    writeln!(out, "{name}_ok:\n  %{name}_next = sub i64 %{name}_remaining, {cost}\n  store i64 %{name}_next, ptr %fuel").unwrap();
}

fn ty(ty: hir::Ty) -> &'static str {
    match ty {
        hir::Ty::Bool => "i1",
        hir::Ty::Unit => "i8",
        hir::Ty::I32 => "i32",
    }
}
fn emit_arithmetic_failure(
    out: &mut Emission,
    guarded: Option<&GuardedDiagnostics>,
    kind: FailureKind,
    span: Span,
    sources: &SourceMap,
    function: usize,
    destination: usize,
) {
    if let Some(diagnostics) = guarded {
        let (id, message) = diagnostics.get(kind, span);
        writeln!(
            out,
            "  call void @__oxid_overflow(ptr @__oxid_guard_error_{id}, i64 {})",
            message.len()
        )
        .unwrap();
    } else {
        let symbol = kind.arithmetic_symbol();
        let length = kind.diagnostic(span, sources).render_human(sources).len();
        writeln!(
            out,
            "  call void @__oxid_overflow(ptr @{symbol}_{function}_{destination}, i64 {length})"
        )
        .unwrap();
    }
    writeln!(out, "  unreachable").unwrap();
}
fn emit(
    program: &Program,
    entry: hir::DefId,
    sources: &SourceMap,
    guarded: Option<&GuardedDiagnostics>,
    fuel: usize,
    out: &mut Emission,
) {
    out.push_str("; Oxid experimental scalar native ABI 1\nsource_filename = \"oxid-native\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_unit()\ndeclare void @__oxid_overflow(ptr, i64) noreturn\ndeclare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)\n");
    // Diagnostics are pre-rendered with the reference renderer. Every UTF-8
    // byte is escaped as LLVM constant data; no source path can become syntax.
    if let Some(diagnostics) = guarded {
        for (id, message) in diagnostics.messages.iter().enumerate() {
            write!(
                out,
                "@__oxid_guard_error_{id} = private unnamed_addr constant [{} x i8] c\"",
                message.len()
            )
            .unwrap();
            for byte in message.bytes() {
                write!(out, "\\{byte:02X}").unwrap();
            }
            out.push_str("\"\n");
        }
    }
    for f in &program.functions {
        for b in &f.blocks {
            for a in b.statements.iter().filter_map(Statement::as_assignment) {
                if let Rvalue::CheckedI32 {
                    op, operator_span, ..
                } = a.value
                {
                    if guarded.is_some() {
                        continue;
                    }
                    for &kind in arithmetic_failures(op) {
                        let symbol = kind.arithmetic_symbol();
                        let message = kind
                            .diagnostic(operator_span, sources)
                            .render_human(sources);
                        write!(
                            out,
                            "@{symbol}_{}_{} = private unnamed_addr constant [{} x i8] c\"",
                            f.id.0,
                            a.destination.0,
                            message.len()
                        )
                        .unwrap();
                        for byte in message.bytes() {
                            write!(out, "\\{byte:02X}").unwrap();
                        }
                        out.push_str("\"\n");
                    }
                }
            }
        }
    }
    for f in &program.functions {
        // Source names and paths never enter LLVM symbol syntax. Deterministic
        // numeric DefIds avoid collisions with main, libc and runtime symbols.
        write!(
            out,
            "\ndefine internal {} @__oxid_fn_{}(",
            ty(f.result),
            f.id.0
        )
        .unwrap();
        if guarded.is_some() {
            out.push_str("ptr %fuel");
        }
        for (i, p) in f.locals.iter().take(f.param_count).enumerate() {
            if i != 0 || guarded.is_some() {
                out.push_str(", ");
            }
            write!(out, "{} %v{i}", ty(p.ty)).unwrap();
        }
        out.push_str(") noinline {\nentry:\n");
        for (i, place) in f.places.iter().enumerate() {
            writeln!(out, "  %p{i} = alloca {}", ty(place.ty)).unwrap();
        }
        writeln!(out, "  br label %b{}", f.entry.0).unwrap();
        // Checked arithmetic splits an OIR block. Phi edges depart from its
        // last successful LLVM block, not necessarily the original bN label.
        let exits: Vec<_> = f
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| {
                if guarded.is_some() {
                    return format!("g{i}_{}_ok", b.statements.len() + 1);
                }
                b.statements
                    .iter()
                    .rev()
                    .filter_map(Statement::as_assignment)
                    .find(|a| matches!(a.value, Rvalue::CheckedI32 { .. }))
                    .map_or_else(
                        || format!("b{i}"),
                        |a| format!("checked{}_ok", a.destination.0),
                    )
            })
            .collect();
        for (i, b) in f.blocks.iter().enumerate() {
            writeln!(out, "b{i}:").unwrap();
            if let Some(merge) = &b.merge {
                let [left, right] = merge.incoming;
                writeln!(
                    out,
                    "  %v{} = phi i1 [ %v{}, %{} ], [ %v{}, %{} ]",
                    merge.destination.0,
                    left.value.local.0,
                    exits[left.predecessor.0],
                    right.value.local.0,
                    exits[right.predecessor.0]
                )
                .unwrap();
            }
            if let (Some(diagnostics), Some(merge)) = (guarded, &b.merge) {
                emit_guard(out, diagnostics, &format!("g{i}_0"), 1, merge.span);
            }
            for (j, statement) in b.statements.iter().enumerate() {
                if let Some(diagnostics) = guarded {
                    emit_guard(
                        out,
                        diagnostics,
                        &format!("g{i}_{}", j + 1),
                        1,
                        statement.span(),
                    );
                }
                let a = match statement {
                    Statement::Assign(a) => a,
                    Statement::Initialize { place, value, .. }
                    | Statement::Store { place, value, .. } => {
                        writeln!(
                            out,
                            "  store {} %v{}, ptr %p{}",
                            ty(f.places[place.id.0].ty),
                            value.local.0,
                            place.id.0
                        )
                        .unwrap();
                        continue;
                    }
                };
                let t = ty(f.locals[a.destination.0].ty);
                #[allow(unreachable_patterns)]
                let rhs = match a.value {
                    Rvalue::Load(place) => {
                        writeln!(
                            out,
                            "  %v{} = load {t}, ptr %p{}",
                            a.destination.0, place.id.0
                        )
                        .unwrap();
                        continue;
                    }
                    Rvalue::Bool(v) => {
                        if v {
                            "true".into()
                        } else {
                            "false".into()
                        }
                    }
                    Rvalue::I32(v) => v.to_string(),
                    Rvalue::Unit => "0".into(),
                    Rvalue::Copy(v) => format!("%v{}", v.local.0),
                    Rvalue::NotBool { operand, .. } => {
                        writeln!(
                            out,
                            "  %v{} = xor i1 %v{}, true",
                            a.destination.0, operand.local.0
                        )
                        .unwrap();
                        continue;
                    }
                    Rvalue::CompareScalar {
                        op, left, right, ..
                    } => {
                        let predicate = match op {
                            hir::ComparisonOp::Equal => "eq",
                            hir::ComparisonOp::NotEqual => "ne",
                            hir::ComparisonOp::Less => "slt",
                            hir::ComparisonOp::LessEqual => "sle",
                            hir::ComparisonOp::Greater => "sgt",
                            hir::ComparisonOp::GreaterEqual => "sge",
                        };
                        // Independent verification admits matching i32/bool only
                        // for equality, and i32 only for signed ordering.
                        let operand_type = ty(f.locals[left.local.0].ty);
                        writeln!(
                            out,
                            "  %v{} = icmp {predicate} {operand_type} %v{}, %v{}",
                            a.destination.0, left.local.0, right.local.0
                        )
                        .unwrap();
                        continue;
                    }
                    Rvalue::CheckedI32 {
                        op,
                        left,
                        right,
                        operator_span,
                    } => {
                        let n = a.destination.0;
                        match op {
                            hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
                                // LLVM signed division and remainder are undefined for
                                // zero and MIN/-1. Neither instruction may precede these
                                // guards, even when only the remainder is requested.
                                writeln!(out, "  %division{n}_zero = icmp eq i32 %v{}, 0\n  br i1 %division{n}_zero, label %division{n}_error, label %division{n}_nonzero\ndivision{n}_error:", right.local.0).unwrap();
                                emit_arithmetic_failure(
                                    out,
                                    guarded,
                                    FailureKind::DivisionByZero,
                                    operator_span,
                                    sources,
                                    f.id.0,
                                    n,
                                );
                                writeln!(out, "division{n}_nonzero:\n  %division{n}_min = icmp eq i32 %v{}, -2147483648\n  %division{n}_negative_one = icmp eq i32 %v{}, -1\n  %overflow{n} = and i1 %division{n}_min, %division{n}_negative_one", left.local.0, right.local.0).unwrap();
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
                                // The intrinsic is defined for every pair of i32 values.
                                // Its wrapped component is only extracted on success.
                                writeln!(out, "  %checked{n} = call {{ i32, i1 }} @llvm.{intrinsic}.with.overflow.i32(i32 %v{}, i32 %v{})", left.local.0, right.local.0).unwrap();
                                writeln!(
                                    out,
                                    "  %overflow{n} = extractvalue {{ i32, i1 }} %checked{n}, 1"
                                )
                                .unwrap();
                            }
                        }
                        writeln!(out, "  br i1 %overflow{n}, label %overflow{n}_error, label %checked{n}_ok\noverflow{n}_error:").unwrap();
                        emit_arithmetic_failure(
                            out,
                            guarded,
                            FailureKind::Overflow,
                            operator_span,
                            sources,
                            f.id.0,
                            n,
                        );
                        writeln!(out, "checked{n}_ok:").unwrap();
                        match op {
                            hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => {
                                let instruction = if op == hir::ArithmeticOp::Divide {
                                    "sdiv"
                                } else {
                                    "srem"
                                };
                                writeln!(
                                    out,
                                    "  %v{n} = {instruction} i32 %v{}, %v{}",
                                    left.local.0, right.local.0
                                )
                                .unwrap();
                            }
                            hir::ArithmeticOp::Add
                            | hir::ArithmeticOp::Subtract
                            | hir::ArithmeticOp::Multiply => {
                                writeln!(
                                    out,
                                    "  %v{n} = extractvalue {{ i32, i1 }} %checked{n}, 0"
                                )
                                .unwrap();
                            }
                        }
                        continue;
                    }
                    _ => unreachable!("native admission allowlist"),
                };
                writeln!(out, "  %v{} = or {t} {rhs}, 0", a.destination.0).unwrap();
            }
            let terminator = b.terminator.as_ref().expect("verified terminator");
            if let Some(diagnostics) = guarded {
                let cost = match &terminator.kind {
                    TerminatorKind::Call { target, args, .. } => {
                        1 + args.len() + program.functions[target.0].slot_count()
                    }
                    _ => 1,
                };
                emit_guard(
                    out,
                    diagnostics,
                    &format!("g{i}_{}", b.statements.len() + 1),
                    cost,
                    terminator.span,
                );
            }
            match &terminator.kind {
                TerminatorKind::Return(v) => {
                    writeln!(out, "  ret {} %v{}", ty(f.result), v.local.0).unwrap();
                }
                TerminatorKind::Goto { target } => {
                    writeln!(out, "  br label %b{}", target.0).unwrap();
                }
                TerminatorKind::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    writeln!(
                        out,
                        "  br i1 %v{}, label %b{}, label %b{}",
                        condition.local.0, then_block.0, else_block.0
                    )
                    .unwrap();
                }
                TerminatorKind::Call {
                    target,
                    args,
                    destination,
                    continuation,
                } => {
                    write!(
                        out,
                        "  %v{} = call {} @__oxid_fn_{}(",
                        destination.0,
                        ty(f.locals[destination.0].ty),
                        target.0
                    )
                    .unwrap();
                    if guarded.is_some() {
                        out.push_str("ptr %fuel");
                    }
                    for (j, arg) in args.iter().enumerate() {
                        if j != 0 || guarded.is_some() {
                            out.push_str(", ");
                        }
                        write!(out, "{} %v{}", ty(f.locals[arg.local.0].ty), arg.local.0).unwrap();
                    }
                    writeln!(out, ")\n  br label %b{}", continuation.0).unwrap();
                }
            }
        }
        out.push_str("}\n");
    }
    let result = program.functions[entry.0].result;
    out.push_str("\ndefine i32 @main() {\nentry:\n");
    if let Some(diagnostics) = guarded {
        writeln!(out, "  %fuel = alloca i64\n  store i64 {}, ptr %fuel", fuel).unwrap();
        let root = &program.functions[entry.0];
        emit_guard(out, diagnostics, "root", 1 + root.slot_count(), root.span);
    }
    writeln!(
        out,
        "  %value = call {} @__oxid_fn_{}({})",
        ty(result),
        entry.0,
        if guarded.is_some() { "ptr %fuel" } else { "" }
    )
    .unwrap();
    match result {
        hir::Ty::Bool => out.push_str(
            "  %wide = zext i1 %value to i32\n  %status = call i32 @__oxid_print_bool(i32 %wide)\n",
        ),
        hir::Ty::I32 => out.push_str("  %status = call i32 @__oxid_print_i32(i32 %value)\n"),
        hir::Ty::Unit => out.push_str("  %status = call i32 @__oxid_print_unit()\n"),
    }
    out.push_str("  ret i32 %status\n}\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser};
    fn verified_with_sources(text: &str) -> (VerifiedProgram, SourceMap) {
        verified_at("native-unit.ox", text)
    }
    fn verified_at(path: &str, text: &str) -> (VerifiedProgram, SourceMap) {
        let mut sources = SourceMap::new();
        let id = sources.add(path.into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
        (lower_and_verify(&typed, &sources).unwrap(), sources)
    }
    fn verified(text: &str) -> VerifiedProgram {
        verified_with_sources(text).0
    }
    #[test]
    fn conservative_cost_counts_both_arms_and_repeated_calls() {
        let p = verified("fn identity(x: bool) -> bool { return x; } fn main() -> bool { if true { identity(false); } else { identity(true); } return false; }");
        let b = p.admit().unwrap();
        // identity: parameter + return temporary + copy + Return.
        assert_eq!(b[0].cost, 4);
        let main = &p.program.functions[1];
        let own = main.locals.len()
            + main
                .blocks
                .iter()
                .map(|b| b.statements.len() + 1)
                .sum::<usize>();
        assert_eq!(b[1].cost, own + 2 * (1 + b[0].cost));
        assert_eq!(b[1].depth, 2);
        assert_eq!(
            b[1].slots,
            main.locals.len() + p.program.functions[0].locals.len()
        );
        assert_eq!(p.run(Some(hir::DefId(1))), Ok(Scalar::Bool(false)));
    }
    #[test]
    fn admission_limits_are_inclusive() {
        let p = verified("fn main() -> () { return; }");
        let span = p.program.functions[0].span;
        for (cap, label) in [
            (MAX_COST, "fuel"),
            (MAX_DEPTH, "depth"),
            (MAX_PARAMS, "parameters"),
            (MAX_FUNCTION_LOCALS, "locals"),
            (MAX_NATIVE_LOCALS, "aggregate locals"),
            (MAX_NATIVE_BLOCKS, "blocks"),
            (MAX_FUNCTIONS, "functions"),
        ] {
            assert!(limit(cap, cap, label, span).is_ok());
            assert!(limit(cap + 1, cap, label, span).is_err());
        }
    }
    #[test]
    fn lowering_uses_private_scalar_abi_and_never_source_identifiers() {
        let (p, sources) = verified_with_sources("fn printf(value: i32) -> i32 { return value; } fn main() -> i32 { return printf(-2147483648); }");
        let ir = p.native_module(Some(hir::DefId(1)), &sources).unwrap();
        assert!(!ir.contains("printf"));
        assert!(ir.contains("define internal i32 @__oxid_fn_0(i32 %v0) noinline"));
        assert!(ir.contains("-2147483648"));
        assert!(!ir.contains("nsw"));
        assert!(!ir.contains("poison"));
        assert!(!ir.contains("undef"));
    }
    #[test]
    fn checked_arithmetic_cost_is_one_including_dead_and_repeated_work() {
        for op in ["+", "-", "*", "/", "%"] {
            let p = verified(&format!("fn main() -> i32 {{ return 1 {op} 2; }}"));
            let bound = p.admit().unwrap()[0];
            // Root unit + 3 slots + 3 assignments (one arithmetic) + Return.
            assert_eq!(1 + bound.cost, 8);
            assert_eq!(bound.slots, 3);
        }
        let p = verified("fn value() -> i32 { return 1 + 2; } fn main() -> i32 { if false { value(); } else { value(); } return value(); }");
        let bounds = p.admit().unwrap();
        let main = &p.program.functions[1];
        let own = main.locals.len()
            + main
                .blocks
                .iter()
                .map(|b| b.statements.len() + 1)
                .sum::<usize>();
        assert_eq!(bounds[0].cost, 7);
        assert_eq!(bounds[1].cost, own + 3 * 7);
    }

    #[test]
    fn checked_lowering_branches_before_exposing_each_result() {
        let (p, sources) = verified_with_sources("fn main() -> i32 { return (1 + 2) * (4 - 3); }");
        let ir = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
        for name in ["sadd", "ssub", "smul"] {
            assert_eq!(
                ir.matches(&format!(
                    "= call {{ i32, i1 }} @llvm.{name}.with.overflow.i32"
                ))
                .count(),
                1
            );
        }
        assert_eq!(ir.matches("  call void @__oxid_overflow(").count(), 3);
        for a in p.program.functions[0].blocks[0]
            .statements
            .iter()
            .filter_map(Statement::as_assignment)
        {
            if matches!(a.value, Rvalue::CheckedI32 { .. }) {
                let n = a.destination.0;
                assert!(ir.contains(&format!(
                    "  br i1 %overflow{n}, label %overflow{n}_error, label %checked{n}_ok"
                )));
                assert!(ir.contains(&format!("  unreachable\nchecked{n}_ok:\n  %v{n} = extractvalue {{ i32, i1 }} %checked{n}, 0")));
            }
        }
        for forbidden in [
            "nsw",
            "nuw",
            "poison",
            "undef",
            " add i32 ",
            " sub i32 ",
            " mul i32 ",
        ] {
            assert!(!ir.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn division_native_guards_precede_instructions_and_preserve_phi_exits() {
        for prefix in ["", "while false {}"] {
            let (program, sources) = verified_with_sources(&format!(
                "fn main()->bool {{ {prefix} return (9/2==4) && (9%2==1); }}"
            ));
            let ir = program
                .native_module(Some(hir::DefId(0)), &sources)
                .unwrap();
            let f = &program.program.functions[0];
            for block in &f.blocks {
                for assign in block.statements.iter().filter_map(Statement::as_assignment) {
                    let Rvalue::CheckedI32 {
                        op, left, right, ..
                    } = assign.value
                    else {
                        continue;
                    };
                    let n = assign.destination.0;
                    let instruction = if op == hir::ArithmeticOp::Divide {
                        "sdiv"
                    } else {
                        "srem"
                    };
                    let zero = ir
                        .find(&format!(
                            "  %division{n}_zero = icmp eq i32 %v{}, 0",
                            right.local.0
                        ))
                        .unwrap();
                    let nonzero = ir.find(&format!("division{n}_nonzero:\n  %division{n}_min = icmp eq i32 %v{}, -2147483648", left.local.0)).unwrap();
                    let overflow = ir
                        .find(&format!(
                            "  %overflow{n} = and i1 %division{n}_min, %division{n}_negative_one"
                        ))
                        .unwrap();
                    let success = ir
                        .find(&format!(
                            "checked{n}_ok:\n  %v{n} = {instruction} i32 %v{}, %v{}",
                            left.local.0, right.local.0
                        ))
                        .unwrap();
                    assert!(zero < nonzero && nonzero < overflow && overflow < success);
                    assert!(ir.contains(&format!("  br i1 %division{n}_zero, label %division{n}_error, label %division{n}_nonzero")));
                    assert!(ir.contains(&format!(
                        "  br i1 %overflow{n}, label %overflow{n}_error, label %checked{n}_ok"
                    )));
                }
                if let Some(merge) = &block.merge {
                    for input in merge.incoming {
                        let predecessor = &f.blocks[input.predecessor.0];
                        let label = if prefix.is_empty() {
                            predecessor
                                .statements
                                .iter()
                                .filter_map(Statement::as_assignment)
                                .rfind(|a| matches!(a.value, Rvalue::CheckedI32 { .. }))
                                .map_or_else(
                                    || format!("b{}", input.predecessor.0),
                                    |a| format!("checked{}_ok", a.destination.0),
                                )
                        } else {
                            format!(
                                "g{}_{}_ok",
                                input.predecessor.0,
                                predecessor.statements.len() + 1
                            )
                        };
                        assert!(ir.contains(&format!("[ %v{}, %{label} ]", input.value.local.0)));
                    }
                }
            }
            assert_eq!(ir.matches(" = sdiv i32 ").count(), 1);
            assert_eq!(ir.matches(" = srem i32 ").count(), 1);
            assert_eq!(program.run(Some(hir::DefId(0))), Ok(Scalar::Bool(true)));
            for forbidden in ["sdiv exact", "srem exact", "nsw", "nuw", "poison", "undef"] {
                assert!(!ir.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn division_native_embeds_both_reference_failures_and_charges_guarded_bytes() {
        for op in ["/", "%"] {
            for prefix in ["", "while false {}"] {
                let (program, sources) = verified_at(
                    "雪\n.ox",
                    &format!("fn main()->i32 {{ {prefix} return 1{op}0; }}"),
                );
                let ir = program
                    .native_module(Some(hir::DefId(0)), &sources)
                    .unwrap();
                let span = program.program.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.statements)
                    .filter_map(Statement::as_assignment)
                    .find_map(|a| match a.value {
                        Rvalue::CheckedI32 { operator_span, .. } => Some(operator_span),
                        _ => None,
                    })
                    .unwrap();
                for kind in [FailureKind::Overflow, FailureKind::DivisionByZero] {
                    let expected = kind.diagnostic(span, &sources).render_human(&sources);
                    let encoded = expected
                        .bytes()
                        .map(|byte| format!("\\{byte:02X}"))
                        .collect::<String>();
                    assert!(ir.contains(&format!("[{} x i8] c\"{encoded}\"", expected.len())));
                }
                if !prefix.is_empty() {
                    let diagnostics = GuardedDiagnostics::new(
                        &program.program,
                        hir::DefId(0),
                        &sources,
                        MAX_GUARDED_DIAGNOSTIC_BYTES,
                    )
                    .unwrap();
                    assert_ne!(
                        diagnostics.get(FailureKind::Overflow, span).0,
                        diagnostics.get(FailureKind::DivisionByZero, span).0
                    );
                    let bytes = diagnostics.messages.iter().map(String::len).sum();
                    assert_eq!(
                        program
                            .native_module_limits(
                                Some(hir::DefId(0)),
                                &sources,
                                execute::MAX_FUEL,
                                bytes,
                                ir.len()
                            )
                            .unwrap(),
                        ir
                    );
                    for (data, llvm, marker) in [
                        (bytes - 1, ir.len(), "diagnostic bytes"),
                        (bytes, ir.len() - 1, "LLVM bytes"),
                    ] {
                        let error = program
                            .native_module_limits(
                                Some(hir::DefId(0)),
                                &sources,
                                execute::MAX_FUEL,
                                data,
                                llvm,
                            )
                            .unwrap_err();
                        assert_eq!(error.code, "E0700");
                        assert!(error.message.contains(marker));
                    }
                }
            }
        }
    }

    #[test]
    fn embedded_overflow_diagnostic_is_reference_human_text_encoded_as_data() {
        let path = "雪\"\\\n\t\u{1b}.ox";
        let text = "// 🦀\r\nfn main() -> i32 { return 2147483647 + 1; }";
        let (p, sources) = verified_at(path, text);
        let ir = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
        assert!(!ir.contains(path));
        let constant = ir
            .lines()
            .find(|line| line.starts_with("@__oxid_error_"))
            .unwrap();
        let encoded = constant
            .split_once(" c\"")
            .unwrap()
            .1
            .strip_suffix('"')
            .unwrap();
        let (chunks, remainder) = encoded.as_bytes().as_chunks::<3>();
        assert!(remainder.is_empty());
        let decoded: Vec<u8> = chunks
            .iter()
            .map(|chunk| {
                assert_eq!(chunk[0], b'\\');
                u8::from_str_radix(std::str::from_utf8(&chunk[1..]).unwrap(), 16).unwrap()
            })
            .collect();
        let failure = p.run(Some(hir::DefId(0))).unwrap_err();
        let expected = failure.diagnostic(&sources).render_human(&sources);
        assert_eq!(decoded, expected.as_bytes());
        assert!(constant.contains(&format!("[{} x i8]", decoded.len())));
        assert!(ir.contains(&format!(", i64 {})", decoded.len())));
        assert_eq!(expected.lines().count(), 2);
        assert!(!expected.contains('\u{1b}'));
        assert!(expected.ends_with(":2:38\n"));
    }
    #[test]
    fn comparison_lowering_uses_signed_i32_and_exact_bool_equality_predicates() {
        let (p, sources) = verified_with_sources("fn main() -> bool { 1 == 2; 1 != 2; 1 < 2; 1 <= 2; 1 > 2; 1 >= 2; true == false; return true != false; }");
        let ir = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
        for predicate in ["eq", "ne", "slt", "sle", "sgt", "sge"] {
            assert_eq!(
                ir.matches(&format!(" = icmp {predicate} i32 ")).count(),
                1,
                "{predicate}"
            );
        }
        for predicate in ["eq", "ne"] {
            assert_eq!(
                ir.matches(&format!(" = icmp {predicate} i1 ")).count(),
                1,
                "{predicate}"
            );
        }
        for forbidden in [
            "icmp ult",
            "icmp ule",
            "icmp ugt",
            "icmp uge",
            " sub i32 ",
            "fcmp",
            "nsw",
            "nuw",
        ] {
            assert!(!ir.contains(forbidden), "{forbidden}");
        }
        assert_eq!(p.run(Some(hir::DefId(0))), Ok(Scalar::Bool(true)));
    }

    #[test]
    fn comparison_native_fuel_bound_counts_one_assignment_for_both_types() {
        for expression in ["1 < 2", "true == false", "false != true"] {
            let p = verified(&format!("fn main() -> bool {{ return {expression}; }}"));
            assert_eq!(1 + p.admit().unwrap()[0].cost, 8);
            assert_eq!(p.admit().unwrap()[0].slots, 3);
        }
        let p = verified("fn main() -> bool { return (1 < 2) == true; }");
        assert_eq!(1 + p.admit().unwrap()[0].cost, 14);
    }
    #[test]
    fn logical_native_cost_counts_entry_merges_and_both_rhs_paths() {
        for (expression, cost) in [
            ("!true", 6),
            ("false && true", 10),
            ("true && false", 10),
            ("true || false", 10),
            ("false || true", 10),
        ] {
            let p = verified(&format!("fn main() -> bool {{ return {expression}; }}"));
            assert_eq!(p.admit().unwrap()[0].cost + 1, cost);
        }
        let p = verified(
            "fn recur() -> bool { return recur(); } fn main() -> bool { return false && recur(); }",
        );
        assert!(p.admit().unwrap_err().message.contains("recursive"));
    }
    #[test]
    fn logical_phi_uses_the_actual_checked_success_predecessor_labels() {
        let (p, sources) =
            verified_with_sources("fn main() -> bool { return !(1+2<4) || (3*4==12) && (1<2); }");
        let module = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
        assert!(module.contains("xor i1"));
        let f = &p.program.functions[0];
        let mut merged = 0;
        let mut split = 0;
        for b in &f.blocks {
            if let Some(m) = &b.merge {
                merged += 1;
                for input in m.incoming {
                    let pred = &f.blocks[input.predecessor.0];
                    let checked = pred
                        .statements
                        .iter()
                        .filter_map(Statement::as_assignment)
                        .rfind(|a| matches!(a.value, Rvalue::CheckedI32 { .. }));
                    let label = match checked {
                        Some(a) => {
                            split += 1;
                            format!("checked{}_ok", a.destination.0)
                        }
                        None => format!("b{}", input.predecessor.0),
                    };
                    assert!(
                        module.contains(&format!("[ %v{}, %{} ]", input.value.local.0, label)),
                        "{module}"
                    );
                }
            }
        }
        assert_eq!(merged, 2);
        assert_eq!(split, 2);
    }
    #[test]
    fn mutable_native_bounds_count_places_and_all_storage_operations() {
        let p = verified("fn main() -> i32 { let mut x = 1; x = 2; return x; }");
        let bound = p.admit().unwrap()[0];
        assert_eq!(bound.slots, 4);
        assert_eq!(1 + bound.cost, 11);
        let p = verified(
            "fn main() -> i32 { let mut x = 1; if false { x = 2; } else { x = 3; } return x; }",
        );
        let f = &p.program.functions[0];
        let own = f.slot_count()
            + f.blocks
                .iter()
                .map(|b| b.statements.len() + 1 + usize::from(b.merge.is_some()))
                .sum::<usize>();
        assert_eq!(p.admit().unwrap()[0].cost, own);
    }
    #[test]
    fn mutable_native_storage_is_private_typed_and_does_not_replace_snapshots() {
        for (ty, initial, next, llvm) in [
            ("bool", "true", "false", "i1"),
            ("i32", "1", "2", "i32"),
            ("()", "()", "()", "i8"),
        ] {
            let (p, sources) = verified_with_sources(&format!("fn main() -> {ty} {{ let mut x = {initial}; let saved = x; x = {next}; return saved; }}"));
            let module = p.native_module(Some(hir::DefId(0)), &sources).unwrap();
            assert_eq!(module.matches(&format!("alloca {llvm}")).count(), 1);
            assert_eq!(module.matches(&format!("load {llvm}, ptr %p0")).count(), 1);
            assert_eq!(module.matches(&format!("store {llvm}")).count(), 2);
            assert_eq!(module.matches("ptr %p0").count(), 3);
            assert!(!module.contains(" undef"));
            assert!(!module.contains(" poison"));
        }
    }
    #[test]
    fn guarded_representation_limits_are_exact_and_preflighted() {
        let (program, sources) = verified_at(
            "path\n雪\t.ox",
            "fn unused()->() { while true {} return; } fn main()->i32 { return 1+2; }",
        );
        let entry = hir::DefId(1);
        let diagnostics = GuardedDiagnostics::new(
            &program.program,
            entry,
            &sources,
            MAX_GUARDED_DIAGNOSTIC_BYTES,
        )
        .unwrap();
        let size: usize = diagnostics.messages.iter().map(String::len).sum();
        let module = program.native_module(Some(entry), &sources).unwrap();
        assert_eq!(
            program
                .native_module_limits(Some(entry), &sources, execute::MAX_FUEL, size, module.len())
                .unwrap(),
            module
        );
        for (data, ir, marker) in [
            (size - 1, module.len(), "diagnostic bytes"),
            (size, module.len() - 1, "LLVM bytes"),
        ] {
            let error = program
                .native_module_limits(Some(entry), &sources, execute::MAX_FUEL, data, ir)
                .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            assert!(error.message.contains(marker));
        }
        for cap in [MAX_GUARDED_DIAGNOSTIC_BYTES, MAX_GUARDED_IR_BYTES] {
            let mut count = LimitedCount {
                len: cap - 1,
                maximum: cap,
            };
            count.write_str("x").unwrap();
            assert_eq!(count.len, cap);
            assert!(count.write_str("x").is_err());
            assert_eq!(count.len, cap);
        }
        // Old unguarded modules do not acquire the guarded representation caps.
        let (acyclic, sources) = verified_with_sources("fn main()->i32 { return 1+2; }");
        assert!(acyclic
            .native_module_limits(Some(hir::DefId(0)), &sources, execute::MAX_FUEL, 0, 0)
            .is_ok());
    }

    #[test]
    fn cyclic_cost_is_unknown_and_propagates_without_weakening_known_costs() {
        let (program, sources) = verified_with_sources(
            "fn leaf()->() { while false {} return; } fn main()->() { leaf(); return; }",
        );
        let bounds = program.admit().unwrap();
        assert!(bounds.iter().all(|b| b.cyclic));
        let module = program
            .native_module(Some(hir::DefId(1)), &sources)
            .unwrap();
        assert!(module.contains("define internal i8 @__oxid_fn_0(ptr %fuel)"));
        assert!(module.contains("store i64 1000000, ptr %fuel"));
        assert_eq!(module.matches("%fuel = alloca i64").count(), 1);
        assert!(!module.contains(" sub nsw "));
        let (returning, sources) =
            verified_with_sources("fn main()->() { while true { return; } return; }");
        assert!(!returning.admit().unwrap()[0].cyclic);
        assert!(!returning
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap()
            .contains("ptr %fuel"));
        let mut text =
            String::from("fn cycle()->() { while false {} return; } fn f0()->() { return; }");
        for n in 1..18 {
            text.push_str(&format!(
                " fn f{n}()->() {{ f{}(); f{}(); return; }}",
                n - 1,
                n - 1
            ));
        }
        text.push_str(" fn main()->() { return; }");
        assert!(verified(&text)
            .admit()
            .unwrap_err()
            .message
            .contains("reference fuel upper bound"));
    }

    #[test]
    fn guarded_phi_uses_terminator_success_labels_and_messages_deduplicate() {
        let (program, sources) = verified_with_sources(
            "fn main()->bool { let mut n=0; while n<2 && (n+1<4) { n=n+1; } return true; }",
        );
        let f = &program.program.functions[0];
        let module = program
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        for block in &f.blocks {
            if let Some(merge) = &block.merge {
                for input in merge.incoming {
                    let predecessor = &f.blocks[input.predecessor.0];
                    assert!(module.contains(&format!(
                        "[ %v{}, %g{}_{}_ok ]",
                        input.value.local.0,
                        input.predecessor.0,
                        predecessor.statements.len() + 1
                    )));
                }
            }
        }
        let messages = GuardedDiagnostics::new(
            &program.program,
            hir::DefId(0),
            &sources,
            MAX_GUARDED_DIAGNOSTIC_BYTES,
        )
        .unwrap();
        assert_eq!(messages.ids.len(), messages.messages.len());
        assert!(
            messages.ids.len()
                < 1 + f
                    .blocks
                    .iter()
                    .map(|b| b.statements.len()
                        + 1
                        + usize::from(b.merge.is_some())
                        + b.statements
                            .iter()
                            .filter(|s| matches!(
                                s.as_assignment(),
                                Some(Assign {
                                    value: Rvalue::CheckedI32 { .. },
                                    ..
                                })
                            ))
                            .count())
                    .sum::<usize>()
        );
    }
    #[test]
    fn actual_guarded_representation_caps_reject_large_verified_raw_modules() {
        let (verified, _) =
            verified_with_sources("fn main()->i32 { let mut n=0; while true { n=n; } return n; }");
        let mut raw = verified.program;
        let f = &mut raw.functions[0];
        let used: usize = f
            .blocks
            .iter()
            .map(|b| b.statements.len() + usize::from(b.merge.is_some()))
            .sum();
        let body = f
            .blocks
            .iter_mut()
            .find(|b| {
                b.statements
                    .iter()
                    .any(|s| matches!(s, Statement::Store { .. }))
            })
            .unwrap();
        let template = body
            .statements
            .iter()
            .find(|s| matches!(s, Statement::Store { .. }))
            .unwrap()
            .clone();
        for offset in 0..MAX_ASSIGNMENTS - used {
            let mut statement = template.clone();
            if let Statement::Store { span, .. } = &mut statement {
                span.start = offset;
                span.end = offset + 1;
            }
            body.statements.push(statement);
        }
        for (path, expected) in [
            ("x".repeat(40), "guarded LLVM bytes"),
            ("x".repeat(200), "guarded diagnostic bytes"),
        ] {
            let mut sources = SourceMap::new();
            sources.add(path, "a".repeat(MAX_ASSIGNMENTS));
            let verified = verify::verify(raw.clone(), &sources).unwrap();
            let error = match verified.native_module(Some(hir::DefId(0)), &sources) {
                Err(error) => error,
                Ok(module) => panic!(
                    "expected {expected} rejection, actual LLVM bytes {}",
                    module.len()
                ),
            };
            assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }
}
