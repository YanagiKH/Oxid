//! Returning scalar-library ABI. No executable emission or runtime is reused.
//! Source authority stays in source::sealed; this child sees immutable verified OIR.
use super::*;
use crate::frontend::{
    declaration_index::{IndexLimits, WorkMeter},
    project::budget::Allocator,
};
use std::mem::size_of;

const OVERFLOW: i32 = 1;
const DIVISION_BY_ZERO: i32 = 2;
const FUEL: i32 = 3;
pub(in crate::frontend::oir) const MAX_LIBRARY_PARAMS: usize = MAX_PARAMS;
const LONGEST_ERROR: &str = "scalar libraries do not support recursive call graphs, including unused functions and unchosen branches";

#[derive(Clone, Copy, Default)]
struct LibraryBound {
    cost: usize,
    depth: usize,
    slots: usize,
    cyclic: bool,
}
struct Graph {
    incoming: [usize; MAX_NATIVE_BLOCKS],
    ready: [usize; MAX_NATIVE_BLOCKS],
}

// The generated formatting argument type is private to std. Match the same
// two-pointer carrier used by the existing qualified scalar emitter inventory.
#[allow(dead_code)]
#[repr(C)]
struct FormatArgument {
    value: *const (),
    formatter: *const (),
}
#[allow(dead_code)]
struct FormatBank {
    returns: [FormatArgument; 16],
    backing: [FormatArgument; 16],
    borrows: [[*const (); 16]; 4],
    arguments: [std::fmt::Arguments<'static>; 3],
    forwarded: [(&'static mut Sink<'static>, std::fmt::Arguments<'static>); 2],
}
#[allow(dead_code)]
struct CompilerBank {
    graph: Graph,
    bounds: [LibraryBound; MAX_FUNCTIONS],
    status: [i32; 5],
    format: FormatBank,
    sinks: [Sink<'static>; 2],
    input: [(
        &'static VerifiedProgram,
        hir::DefId,
        &'static SourceMap,
        usize,
        IndexLimits,
        &'static WorkMeter,
        &'static mut Allocator,
    ); 2],
    functions: [&'static Function; 3],
    blocks: [&'static BasicBlock; 3],
    statements: [&'static Statement; 3],
    assignments: [&'static Assign; 3],
    terminators: [&'static Terminator; 3],
    node_ids: [usize; 16],
    arithmetic: [Option<usize>; 8],
    returned: [Result<String, Box<Diagnostic>>; 3],
    scalar_names: [&'static str; 4],
    operand: [Operand; 3],
    call_inputs: [(&'static Function, &'static str, usize, usize); 2],
    node_inputs: [(&'static mut Sink<'static>, Span); 2],
    admission_inputs: [(&'static Program, &'static WorkMeter, Span); 2],
    function_iterators: [std::slice::Iter<'static, Function>; 2],
    block_iterators: [std::slice::Iter<'static, BasicBlock>; 3],
    statement_iterators: [std::slice::Iter<'static, Statement>; 2],
    local_iterator: std::slice::Iter<'static, LocalDecl>,
    place_iterator: std::slice::Iter<'static, PlaceDecl>,
    argument_iterators: [std::slice::Iter<'static, Operand>; 2],
    failure_payloads: [Diagnostic; 2],
    failure_messages: [[u8; LONGEST_ERROR.len()]; 2],
}
pub(in crate::frontend::oir) const CONTROL_BYTES: usize = size_of::<CompilerBank>();

fn error(message: &'static str) -> Box<Diagnostic> {
    let message = if message.len() <= LONGEST_ERROR.len() {
        message
    } else {
        "internal compiler error: unpriced library diagnostic"
    };
    Diagnostic::new("E0700", "native-library", message, None)
}
fn visit(work: &WorkMeter, origin: Span) -> Result<(), Box<Diagnostic>> {
    // One closed node dispatch plus its fixed field/checked-arithmetic roles.
    // Output byte/format visits have their own exact debits in Sink::write_str.
    work.debit(128, origin, "scalar library node")
}
fn narrow(ty: hir::Ty) -> bool {
    matches!(ty, hir::Ty::Bool | hir::Ty::I32 | hir::Ty::Unit)
}

fn cyclic(f: &Function, graph: &mut Graph, work: &WorkMeter) -> Result<bool, Box<Diagnostic>> {
    graph.incoming[..f.blocks.len()].fill(0);
    for block in &f.blocks {
        visit(work, block.span)?;
        for target in successors(&block.terminator.as_ref().expect("verified terminator").kind) {
            visit(work, block.span)?;
            graph.incoming[target.0] += 1;
        }
    }
    let mut end = 0;
    for (id, &count) in graph.incoming[..f.blocks.len()].iter().enumerate() {
        visit(work, f.span)?;
        if count == 0 {
            graph.ready[end] = id;
            end += 1;
        }
    }
    let mut next = 0;
    while next < end {
        let id = graph.ready[next];
        next += 1;
        visit(work, f.span)?;
        for target in successors(
            &f.blocks[id]
                .terminator
                .as_ref()
                .expect("verified terminator")
                .kind,
        ) {
            visit(work, f.blocks[id].span)?;
            graph.incoming[target.0] -= 1;
            if graph.incoming[target.0] == 0 {
                graph.ready[end] = target.0;
                end += 1;
            }
        }
    }
    Ok(next != f.blocks.len())
}

fn admission(program: &Program, work: &WorkMeter, origin: Span) -> Result<(), Box<Diagnostic>> {
    limit(
        program.functions.len(),
        MAX_FUNCTIONS,
        "function count",
        origin,
    )?;
    let mut graph = Graph {
        incoming: [0; MAX_NATIVE_BLOCKS],
        ready: [0; MAX_NATIVE_BLOCKS],
    };
    let mut bounds = [LibraryBound::default(); MAX_FUNCTIONS];
    let (mut locals, mut blocks) = (0usize, 0usize);
    for f in &program.functions {
        visit(work, f.span)?;
        limit(f.param_count, MAX_PARAMS, "parameter count", f.span)?;
        limit(
            f.slot_count(),
            MAX_FUNCTION_LOCALS,
            "locals per function",
            f.span,
        )?;
        locals = locals
            .checked_add(f.slot_count())
            .ok_or_else(|| error("library local count overflow"))?;
        blocks = blocks
            .checked_add(f.blocks.len())
            .ok_or_else(|| error("library block count overflow"))?;
        limit(blocks, MAX_NATIVE_BLOCKS, "aggregate blocks", origin)?;
        for local in &f.locals {
            visit(work, local.span)?;
            if !narrow(local.ty) {
                return Err(error(
                    "scalar libraries support only bool, i32 and unit helper bodies",
                ));
            }
        }
        for place in &f.places {
            visit(work, place.span)?;
            if !narrow(place.ty) {
                return Err(error(
                    "scalar libraries support only bool, i32 and unit helper bodies",
                ));
            }
        }
        if !narrow(f.result) {
            return Err(error(
                "scalar libraries support only bool, i32 and unit helper bodies",
            ));
        }
        for block in &f.blocks {
            visit(work, block.span)?;
            for statement in &block.statements {
                visit(work, statement.span())?;
                if let Statement::Assign(a) = statement {
                    // The shared scalar type-admission helper's existing
                    // independently derived tariff is 4*128 (RFC0030).
                    work.debit(4 * 128, a.span, "library scalar type admission")?;
                    admit_assignment(f, a)?;
                    if !matches!(
                        a.value,
                        Rvalue::Bool(_)
                            | Rvalue::I32(_)
                            | Rvalue::Unit
                            | Rvalue::Copy(_)
                            | Rvalue::Load(_)
                            | Rvalue::NotBool { .. }
                            | Rvalue::CompareScalar { .. }
                            | Rvalue::CheckedI32 { .. }
                            | Rvalue::CheckedNegateI32 { .. }
                    ) {
                        return Err(error("scalar library operation is unsupported"));
                    }
                }
            }
        }
    }
    limit(locals, MAX_NATIVE_LOCALS, "aggregate locals", origin)?;
    // Allocation-free reverse topological admission. An unused recursive call
    // graph is rejected exactly as for the old bounded scalar native consumer.
    let mut complete = 0;
    while complete < program.functions.len() {
        let previous = complete;
        for f in &program.functions {
            visit(work, f.span)?;
            if bounds[f.id.0].depth != 0 {
                continue;
            }
            let mut ready = true;
            for block in &f.blocks {
                visit(work, block.span)?;
                if let TerminatorKind::Call { target, .. } =
                    &block.terminator.as_ref().expect("verified terminator").kind
                {
                    ready &= bounds[target.0].depth != 0;
                }
            }
            if !ready {
                continue;
            }
            let mut bound = LibraryBound {
                cost: f.slot_count(),
                depth: 1,
                slots: f.slot_count(),
                cyclic: cyclic(f, &mut graph, work)?,
            };
            for block in &f.blocks {
                visit(work, block.span)?;
                bound.cost = bound.cost.saturating_add(
                    block.statements.len() + 1 + usize::from(block.merge.is_some()),
                );
                if let TerminatorKind::Call { target, args, .. } =
                    &block.terminator.as_ref().expect("verified terminator").kind
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
            bounds[f.id.0] = bound;
            complete += 1;
        }
        if previous == complete {
            return Err(error("scalar libraries do not support recursive call graphs, including unused functions and unchosen branches"));
        }
    }
    Ok(())
}

struct Sink<'a> {
    text: Option<String>,
    length: usize,
    maximum: usize,
    work: &'a WorkMeter,
    origin: Span,
    failure: Option<Box<Diagnostic>>,
}
impl Sink<'_> {
    fn node(&mut self, origin: Span) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        if let Err(error) = visit(self.work, origin) {
            self.failure = Some(error);
            return Err(std::fmt::Error);
        }
        Ok(())
    }
}
impl Write for Sink<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        let next = self
            .length
            .checked_add(text.len())
            .filter(|&n| n <= self.maximum);
        if next.is_none()
            || self
                .text
                .as_ref()
                .is_some_and(|s| next.unwrap() > s.capacity())
        {
            self.failure = Some(error("library LLVM byte budget exceeded"));
            return Err(std::fmt::Error);
        }
        if let Err(e) = self
            .work
            .debit(1 + text.len() as u64, self.origin, "scalar library text")
        {
            self.failure = Some(e);
            return Err(std::fmt::Error);
        }
        self.length = next.unwrap();
        if let Some(output) = &mut self.text {
            output.push_str(text);
        }
        Ok(())
    }
}

#[cfg(test)]
fn module(
    witness: &VerifiedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    fuel: usize,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<String, Box<Diagnostic>> {
    module_with_limits(
        witness,
        entry,
        sources,
        fuel,
        IndexLimits::default(),
        work,
        allocator,
    )
}

pub(in crate::frontend::oir) fn module_with_limits(
    witness: &VerifiedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    fuel: usize,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<String, Box<Diagnostic>> {
    crate::frontend::native_library::require_host()?;
    if fuel > execute::MAX_FUEL {
        return Err(error("library fuel exceeds the existing bounded maximum"));
    }
    let root = witness
        .program
        .functions
        .get(entry.0)
        .filter(|f| f.id == entry)
        .ok_or_else(|| error("invalid sealed library export"))?;
    if root.result != hir::Ty::I32
        || root
            .locals
            .iter()
            .take(root.param_count)
            .any(|p| p.ty != hir::Ty::I32)
        || !sources.is_valid_span(root.span)
    {
        return Err(error(
            "library export requires an authenticated i32 signature",
        ));
    }
    let retained = usize::try_from(limits.retained.min(IndexLimits::default().retained))
        .map_err(|_| error("library carrier size overflow"))?;
    if CONTROL_BYTES > limits.scratch.min(IndexLimits::default().scratch) as usize {
        return Err(error("library native carrier budget exceeded"));
    }
    let maximum = MAX_GUARDED_IR_BYTES.min(
        retained
            .checked_sub(CONTROL_BYTES)
            .ok_or_else(|| error("library native carrier budget exceeded"))?,
    );
    admission(&witness.program, work, root.span)?;
    let mut count = Sink {
        text: None,
        length: 0,
        maximum,
        work,
        origin: root.span,
        failure: None,
    };
    let result = emit_library(&witness.program, entry, fuel, &mut count);
    if let Some(e) = count.failure {
        return Err(e);
    }
    result.map_err(|_| error("library count emission failed"))?;
    let expected = count.length;
    let mut text = String::new();
    allocator
        .string(&mut text, expected, "scalar library LLVM")
        .map_err(|_| error("cannot allocate scalar library LLVM"))?;
    // Capacity excess cannot be borrowed from any old bank.
    if text
        .capacity()
        .checked_add(CONTROL_BYTES)
        .is_none_or(|n| n > retained)
    {
        return Err(error("library LLVM capacity exceeds paid carrier budget"));
    }
    let mut render = Sink {
        text: Some(text),
        length: 0,
        maximum: expected,
        work,
        origin: root.span,
        failure: None,
    };
    let result = emit_library(&witness.program, entry, fuel, &mut render);
    if let Some(e) = render.failure {
        return Err(e);
    }
    result.map_err(|_| error("library render emission failed"))?;
    if render.length != expected {
        return Err(error("library count/render mismatch"));
    }
    Ok(render.text.expect("render owns admitted string"))
}

fn failure(out: &mut Sink<'_>, result: hir::Ty, status: i32) -> std::fmt::Result {
    writeln!(
        out,
        "  ret {{ i32, {} }} {{ i32 {status}, {} 0 }}",
        ty(result),
        ty(result)
    )
}
fn guard(
    out: &mut Sink<'_>,
    f: &Function,
    block: usize,
    operation: usize,
    cost: usize,
) -> std::fmt::Result {
    writeln!(out, "  %g{block}_{operation} = call i1 @__oxid_library_debit(ptr %fuel, i64 {cost})\n  br i1 %g{block}_{operation}, label %g{block}_{operation}_ok, label %g{block}_{operation}_failed\ng{block}_{operation}_failed:")?;
    failure(out, f.result, FUEL)?;
    writeln!(out, "g{block}_{operation}_ok:")
}

fn assignment(out: &mut Sink<'_>, f: &Function, a: &Assign) -> std::fmt::Result {
    out.node(a.span)?;
    let n = a.destination.0;
    let t = ty(f.locals[n].ty);
    match a.value {
        Rvalue::Bool(v) => writeln!(out, "  %v{n} = or i1 {}, false", u8::from(v)),
        Rvalue::I32(v) => writeln!(out, "  %v{n} = or i32 {v}, 0"),
        Rvalue::Unit => writeln!(out, "  %v{n} = or i8 0, 0"),
        Rvalue::Copy(v) => writeln!(out, "  %v{n} = or {t} %v{}, 0", v.local.0),
        Rvalue::Load(place) => writeln!(out, "  %v{n} = load {t}, ptr %p{}", place.id.0),
        Rvalue::NotBool { operand, .. } => {
            writeln!(out, "  %v{n} = xor i1 %v{}, true", operand.local.0)
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
            writeln!(
                out,
                "  %v{n} = icmp {predicate} {} %v{}, %v{}",
                ty(f.locals[left.local.0].ty),
                left.local.0,
                right.local.0
            )
        }
        Rvalue::CheckedNegateI32 { operand, .. } => {
            writeln!(
                out,
                "  %checked{n} = call {{ i32, i1 }} @llvm.ssub.with.overflow.i32(i32 0, i32 %v{})",
                operand.local.0
            )?;
            arithmetic_result(out, f, n)
        }
        Rvalue::CheckedI32 {
            op, left, right, ..
        } => {
            if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) {
                writeln!(out, "  %zero{n} = icmp eq i32 %v{}, 0\n  br i1 %zero{n}, label %division{n}_failed, label %division{n}_nonzero\ndivision{n}_failed:", right.local.0)?;
                failure(out, f.result, DIVISION_BY_ZERO)?;
                writeln!(out, "division{n}_nonzero:\n  %min{n} = icmp eq i32 %v{}, -2147483648\n  %minus_one{n} = icmp eq i32 %v{}, -1\n  %overflow{n} = and i1 %min{n}, %minus_one{n}\n  br i1 %overflow{n}, label %overflow{n}_failed, label %checked{n}_ok\noverflow{n}_failed:", left.local.0, right.local.0)?;
                failure(out, f.result, OVERFLOW)?;
                let instruction = if op == hir::ArithmeticOp::Divide {
                    "sdiv"
                } else {
                    "srem"
                };
                writeln!(
                    out,
                    "checked{n}_ok:\n  %v{n} = {instruction} i32 %v{}, %v{}",
                    left.local.0, right.local.0
                )
            } else {
                let intrinsic = match op {
                    hir::ArithmeticOp::Add => "sadd",
                    hir::ArithmeticOp::Subtract => "ssub",
                    hir::ArithmeticOp::Multiply => "smul",
                    _ => unreachable!("closed arithmetic"),
                };
                writeln!(out, "  %checked{n} = call {{ i32, i1 }} @llvm.{intrinsic}.with.overflow.i32(i32 %v{}, i32 %v{})", left.local.0, right.local.0)?;
                arithmetic_result(out, f, n)
            }
        }
        _ => unreachable!("closed library admission"),
    }
}

fn arithmetic_result(out: &mut Sink<'_>, f: &Function, n: usize) -> std::fmt::Result {
    writeln!(out, "  %overflow{n} = extractvalue {{ i32, i1 }} %checked{n}, 1\n  br i1 %overflow{n}, label %overflow{n}_failed, label %checked{n}_ok\noverflow{n}_failed:")?;
    failure(out, f.result, OVERFLOW)?;
    writeln!(
        out,
        "checked{n}_ok:\n  %v{n} = extractvalue {{ i32, i1 }} %checked{n}, 0"
    )
}

fn function(out: &mut Sink<'_>, program: &Program, f: &Function) -> std::fmt::Result {
    out.node(f.span)?;
    let t = ty(f.result);
    write!(
        out,
        "define internal {{ i32, {t} }} @__oxid_library_fn_{}(ptr %fuel",
        f.id.0
    )?;
    for (i, p) in f.locals.iter().take(f.param_count).enumerate() {
        out.node(p.span)?;
        write!(out, ", {} %v{i}", ty(p.ty))?;
    }
    writeln!(out, ") noinline {{\nentry:")?;
    for (i, place) in f.places.iter().enumerate() {
        out.node(place.span)?;
        writeln!(out, "  %p{i} = alloca {}", ty(place.ty))?;
    }
    writeln!(out, "  br label %b{}", f.entry.0)?;
    for (i, block) in f.blocks.iter().enumerate() {
        out.node(block.span)?;
        writeln!(out, "b{i}:")?;
        if let Some(merge) = &block.merge {
            out.node(merge.span)?;
            let [a, b] = merge.incoming;
            writeln!(
                out,
                "  %v{} = phi i1 [ %v{}, %exit{} ], [ %v{}, %exit{} ]",
                merge.destination.0,
                a.value.local.0,
                a.predecessor.0,
                b.value.local.0,
                b.predecessor.0
            )?;
            guard(out, f, i, 0, 1)?;
        }
        for (j, statement) in block.statements.iter().enumerate() {
            out.node(statement.span())?;
            guard(out, f, i, j + 1, 1)?;
            match statement {
                Statement::Assign(a) => assignment(out, f, a)?,
                Statement::Initialize { place, value, .. }
                | Statement::Store { place, value, .. } => writeln!(
                    out,
                    "  store {} %v{}, ptr %p{}",
                    ty(f.places[place.id.0].ty),
                    value.local.0,
                    place.id.0
                )?,
            }
        }
        let terminator = block.terminator.as_ref().expect("verified terminator");
        out.node(terminator.span)?;
        let term = &terminator.kind;
        let cost = if let TerminatorKind::Call { target, args, .. } = term {
            1 + args.len() + program.functions[target.0].slot_count()
        } else {
            1
        };
        guard(out, f, i, block.statements.len() + 1, cost)?;
        if let TerminatorKind::Call {
            target,
            args,
            destination,
            ..
        } = term
        {
            let child = &program.functions[target.0];
            let ct = ty(child.result);
            write!(
                out,
                "  %call{i} = call {{ i32, {ct} }} @__oxid_library_fn_{}(ptr %fuel",
                target.0
            )?;
            for arg in args {
                out.node(arg.span)?;
                write!(out, ", {} %v{}", ty(f.locals[arg.local.0].ty), arg.local.0)?;
            }
            writeln!(out, ")\n  %status{i} = extractvalue {{ i32, {ct} }} %call{i}, 0\n  %success{i} = icmp eq i32 %status{i}, 0\n  br i1 %success{i}, label %exit{i}, label %call{i}_failed\ncall{i}_failed:\n  %failed{i} = insertvalue {{ i32, {t} }} zeroinitializer, i32 %status{i}, 0\n  ret {{ i32, {t} }} %failed{i}\nexit{i}:\n  %v{} = extractvalue {{ i32, {ct} }} %call{i}, 1", destination.0)?;
        } else {
            writeln!(out, "  br label %exit{i}\nexit{i}:")?;
        }
        match term {
            TerminatorKind::Return(value) => writeln!(out, "  %returned{i} = insertvalue {{ i32, {t} }} zeroinitializer, {t} %v{}, 1\n  ret {{ i32, {t} }} %returned{i}", value.local.0)?,
            TerminatorKind::Goto { target } => writeln!(out, "  br label %b{}", target.0)?,
            TerminatorKind::Branch { condition, then_block, else_block } => writeln!(out, "  br i1 %v{}, label %b{}, label %b{}", condition.local.0, then_block.0, else_block.0)?,
            TerminatorKind::Call { continuation, .. } => writeln!(out, "  br label %b{}", continuation.0)?,
        }
    }
    writeln!(out, "}}")
}

fn emit_library(
    program: &Program,
    entry: hir::DefId,
    fuel: usize,
    out: &mut Sink<'_>,
) -> std::fmt::Result {
    writeln!(out, "; Oxid scalar library ABI 1\nsource_filename = \"oxid-library\"\ntarget triple = \"{}\"\ndeclare {{ i32, i1 }} @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare {{ i32, i1 }} @llvm.ssub.with.overflow.i32(i32, i32)\ndeclare {{ i32, i1 }} @llvm.smul.with.overflow.i32(i32, i32)", crate::frontend::build_config::NativeBuildSettings::new(false).target_triple())?;
    out.write_str(DEBIT)?;
    for f in &program.functions {
        function(out, program, f)?;
    }
    let root = &program.functions[entry.0];
    out.write_str("define i32 @oxid_library_abi_version() {\nentry:\n  ret i32 1\n}\ndefine i32 @oxid_library_call_v1(")?;
    for i in 0..root.param_count {
        write!(out, "i32 %arg{i}, ")?;
    }
    out.write_str("ptr %out) {\nentry:\n  %null = icmp eq ptr %out, null\n  %address = ptrtoint ptr %out to i64\n  %low_bits = and i64 %address, 3\n  %misaligned = icmp ne i64 %low_bits, 0\n  %invalid = or i1 %null, %misaligned\n  br i1 %invalid, label %invalid_output, label %begin\ninvalid_output:\n  ret i32 4\nbegin:\n")?;
    writeln!(out, "  %fuel = alloca i64, align 8\n  store i64 {fuel}, ptr %fuel, align 8\n  %root_ready = call i1 @__oxid_library_debit(ptr %fuel, i64 {})\n  br i1 %root_ready, label %invoke, label %exhausted\nexhausted:\n  ret i32 {FUEL}\ninvoke:\n  %answer = call {{ i32, i32 }} @__oxid_library_fn_{}(ptr %fuel", 1 + root.slot_count(), entry.0)?;
    // Continue the call started above, without introducing another return ABI.
    // The newline is LLVM whitespace, including before each argument comma.
    for i in 0..root.param_count {
        write!(out, ", i32 %arg{i}")?;
    }
    out.write_str(")\n  %status = extractvalue { i32, i32 } %answer, 0\n  %ok = icmp eq i32 %status, 0\n  br i1 %ok, label %publish, label %failed\nfailed:\n  ret i32 %status\npublish:\n  %value = extractvalue { i32, i32 } %answer, 1\n  store i32 %value, ptr %out, align 4\n  ret i32 0\n}\n")
}

const DEBIT: &str = "define internal i1 @__oxid_library_debit(ptr %fuel, i64 %cost) {\nentry:\n  %remaining = load i64, ptr %fuel, align 8\n  %ready = icmp uge i64 %remaining, %cost\n  br i1 %ready, label %charge, label %failed\nfailed:\n  ret i1 false\ncharge:\n  %next = sub i64 %remaining, %cost\n  store i64 %next, ptr %fuel, align 8\n  ret i1 true\n}\n";

#[cfg(test)]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    use super::*;

    fn verified(text: &str) -> (VerifiedProgram, SourceMap) {
        super::super::tests::verified_with_sources(text)
    }

    #[test]
    fn literal_fuel_has_independent_four_and_three_boundaries() {
        let (program, sources) = verified("fn main()->i32{return 42;}");
        let root = &program.program.functions[0];
        // Independent logical-machine oracle: activation 1+one slot, one
        // literal statement, one return. No emitted counter is the oracle.
        assert_eq!(root.slot_count(), 1);
        assert_eq!(root.blocks.len(), 1);
        assert_eq!(root.blocks[0].statements.len(), 1);
        assert!(matches!(
            root.blocks[0].statements[0],
            Statement::Assign(Assign {
                value: Rvalue::I32(42),
                ..
            })
        ));
        assert_eq!(
            execute::run_with_fuel(&program, hir::DefId(0), 4),
            Ok(Scalar::I32(42))
        );
        assert!(matches!(
            execute::run_with_fuel(&program, hir::DefId(0), 3),
            Err(RunFailure::Fuel(_))
        ));
        for fuel in [4, 3] {
            let work = WorkMeter::new(IndexLimits::default().work);
            let text = module(
                &program,
                hir::DefId(0),
                &sources,
                fuel,
                &work,
                &mut Allocator::default(),
            )
            .unwrap();
            assert!(text.contains(&format!("store i64 {fuel}, ptr %fuel")));
            assert!(text.contains("@__oxid_library_debit(ptr %fuel, i64 2)"));
            assert!(text.contains("ret { i32, i32 } { i32 3, i32 0 }"));
            for forbidden in [
                "@main",
                "noreturn",
                "unreachable",
                "__oxid_overflow",
                "__oxid_process",
                "__oxid_bytebuf",
                "thread_local",
            ] {
                assert!(!text.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn nested_errors_propagate_before_result_extraction_or_publication() {
        let (program, sources) =
            verified("fn divide(x:i32)->i32{return 10/x;}fn main()->i32{return divide(0)+1;}");
        assert!(matches!(
            program.run(Some(hir::DefId(1))),
            Err(RunFailure::DivisionByZero(_))
        ));
        let work = WorkMeter::new(IndexLimits::default().work);
        let text = module(
            &program,
            hir::DefId(1),
            &sources,
            execute::MAX_FUEL,
            &work,
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(text.contains("ret { i32, i32 } { i32 2, i32 0 }"));
        assert!(text.contains("ret { i32, i32 } %failed"));
        assert_eq!(
            text.matches("store i32 %value, ptr %out, align 4").count(),
            1
        );
        let wrapper = text
            .split("define i32 @oxid_library_call_v1")
            .nth(1)
            .unwrap();
        assert!(
            wrapper.find("failed:\n  ret i32 %status").unwrap() < wrapper.find("publish:").unwrap()
        );
        assert_eq!(
            wrapper.matches("store i64 ").count(),
            1,
            "one invocation fuel word"
        );
        assert!(!text.contains("call void"));
    }

    #[test]
    fn helper_graphs_keep_bounded_recursion_denial_and_unused_admission() {
        let (program, sources) =
            verified("fn recurse()->i32{return recurse();}fn main()->i32{return 42;}");
        let work = WorkMeter::new(IndexLimits::default().work);
        let mut allocator = Allocator::default();
        let result = module(
            &program,
            hir::DefId(1),
            &sources,
            execute::MAX_FUEL,
            &work,
            &mut allocator,
        );
        assert!(result
            .unwrap_err()
            .message
            .contains("recursive call graphs"));
        assert_eq!(allocator.attempts, 0);
    }

    #[test]
    fn emission_reservation_failure_returns_without_a_partial_module() {
        let (program, sources) = verified("fn main()->i32{return 42;}");
        let work = WorkMeter::new(IndexLimits::default().work);
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        };
        assert!(module(
            &program,
            hir::DefId(0),
            &sources,
            execute::MAX_FUEL,
            &work,
            &mut allocator
        )
        .is_err());
        assert_eq!(allocator.attempts, 1);
    }

    #[test]
    fn explicit_carriers_and_output_have_exact_and_one_less_admission() {
        let (program, sources) = verified("fn main()->i32{return 42;}");
        let work = WorkMeter::new(IndexLimits::default().work);
        let rendered = module(
            &program,
            hir::DefId(0),
            &sources,
            4,
            &work,
            &mut Allocator::default(),
        )
        .unwrap();
        // Endpoint is the complete, independently named carrier bank plus the
        // actual counted output, never a newly raised absolute memory limit.
        let exact = CONTROL_BYTES + rendered.len();
        let limits = IndexLimits {
            retained: exact as u64,
            scratch: CONTROL_BYTES as u64,
            ..IndexLimits::default()
        };
        let work = WorkMeter::new(limits.work);
        let actual = module_with_limits(
            &program,
            hir::DefId(0),
            &sources,
            4,
            limits,
            &work,
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(actual, rendered);
        for lower in [
            IndexLimits {
                retained: exact as u64 - 1,
                ..limits
            },
            IndexLimits {
                scratch: CONTROL_BYTES as u64 - 1,
                ..limits
            },
            IndexLimits {
                retained: 0,
                scratch: 0,
                ..limits
            },
        ] {
            let work = WorkMeter::new(lower.work);
            let mut allocator = Allocator::default();
            assert!(module_with_limits(
                &program,
                hir::DefId(0),
                &sources,
                4,
                lower,
                &work,
                &mut allocator
            )
            .is_err());
            assert_eq!(allocator.attempts, 0, "refusal precedes output reservation");
        }
    }
}
