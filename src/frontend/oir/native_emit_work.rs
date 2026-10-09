//! Disconnected paid scalar-OIR dimension scan for private Emit feasibility.
//!
//! This module is compiled but not called by any native/importer route. It does
//! not calculate or authorize native-body work, allocate output, or enable
//! private Emit. The caller must separately pay the complete named carrier
//! inventory before entering this scan; no storage admission is made here.
use super::*;
use crate::frontend::declaration_index::WorkMeter;
use std::mem::{size_of, size_of_val};

const SETUP_WORK: u64 = 4_096;
const FUNCTION_WORK: u64 = 128;
const BLOCK_WORK: u64 = 256;
const STATEMENT_WORK: u64 = 128;

/// These are actual immutable lengths/counts, not capacities or HIR guesses.
/// They carry no owner, verified witness, callback, or native admission proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend::oir) struct Dimensions {
    pub(in crate::frontend::oir) functions: u64,
    pub(in crate::frontend::oir) locals: u64,
    pub(in crate::frontend::oir) places: u64,
    pub(in crate::frontend::oir) parameters: u64,
    pub(in crate::frontend::oir) blocks: u64,
    pub(in crate::frontend::oir) statements: u64,
    pub(in crate::frontend::oir) merges: u64,
    pub(in crate::frontend::oir) calls: u64,
    pub(in crate::frontend::oir) arguments: u64,
    pub(in crate::frontend::oir) edges: u64,
    // Historical field name: counts all checked scalar failure occurrences,
    // including the single range failure of checked byte narrowing.
    pub(in crate::frontend::oir) arithmetic_failures: u64,
    /// Selects the longer E0610 diagnostic envelope without repricing old inputs.
    pub(in crate::frontend::oir) has_byte_range_failure: bool,
    /// Any successor <= its predecessor's enumeration index. A backward edge
    /// is only a conservative work selector, never evidence of native admission
    /// or even proof that the function's CFG contains a cycle.
    pub(in crate::frontend::oir) maybe_cyclic: bool,
}

/// Fixed inputs copied from the caller's already-paid HIR-derived plan. This
/// data is not a replacement plan and does not itself grant admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) struct Upper {
    pub(in crate::frontend::oir) functions: u64,
    pub(in crate::frontend::oir) blocks: u64,
    pub(in crate::frontend::oir) slots: u64,
    pub(in crate::frontend::oir) definitions: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) enum Failure {
    Work,
    Overflow,
    UpperMismatch,
    Invariant,
}

fn add(left: u64, right: u64) -> Result<u64, Failure> {
    left.checked_add(right).ok_or(Failure::Overflow)
}

fn length(value: usize) -> Result<u64, Failure> {
    u64::try_from(value).map_err(|_| Failure::Overflow)
}

fn add_length(total: u64, value: usize) -> Result<u64, Failure> {
    add(total, length(value)?)
}

fn reconcile(dimensions: &Dimensions, upper: Upper) -> Result<(), Failure> {
    let slots = add(dimensions.locals, dimensions.places)?;
    if dimensions.functions > upper.functions
        || dimensions.blocks > upper.blocks
        || slots > upper.slots
        || dimensions.statements > upper.definitions
    {
        return Err(Failure::UpperMismatch);
    }
    Ok(())
}

/// WorkMeter is the caller's original non-Sync Cell-backed meter. Nothing
/// between this precheck and debit can change it. Therefore its allocating
/// diagnostic failure branches are unreachable on this call; ordinary scan
/// denial returns a fixed Failure before invoking them. No fresh meter, reset,
/// refund, source inspection, or diagnostic construction is introduced.
pub(in crate::frontend::oir) fn debit(
    work: &WorkMeter,
    units: u64,
    origin: Span,
    operation: &'static str,
) -> Result<(), Failure> {
    let remaining = work.limit().checked_sub(work.used()).ok_or(Failure::Work)?;
    if units > remaining {
        return Err(Failure::Work);
    }
    work.debit(units, origin, operation)
        .map_err(|_| Failure::Invariant)
}

fn arithmetic_failures(statement: &Statement) -> (u64, bool) {
    match statement {
        Statement::Assign(assign) => match &assign.value {
            Rvalue::CheckedI32ToU8 { .. } => (1, true),
            Rvalue::CheckedNegateI32 { .. } => (1, false),
            Rvalue::CheckedI32 { op, .. } => match op {
                hir::ArithmeticOp::Add
                | hir::ArithmeticOp::Subtract
                | hir::ArithmeticOp::Multiply => (1, false),
                hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder => (2, false),
            },
            Rvalue::U8ToI32 { .. }
            | Rvalue::Load(_)
            | Rvalue::NotBool { .. }
            | Rvalue::Bool(_)
            | Rvalue::I32(_)
            | Rvalue::Unit
            | Rvalue::Copy(_)
            | Rvalue::CompareScalar { .. } => (0, false),
        },
        Statement::Initialize { .. } | Statement::Store { .. } => (0, false),
    }
}

/// Each inspection follows its own paid boundary. The setup covers the fixed
/// function-vector length access and final result handling. Vector cursors only
/// yield borrowed rows before the corresponding debit; no row field is read
/// until payment succeeds. Local/place/argument elements are never visited.
pub(in crate::frontend::oir) fn scan(
    verified: &VerifiedProgram,
    upper: Upper,
    work: &WorkMeter,
    origin: Span,
) -> Result<Dimensions, Failure> {
    debit(work, SETUP_WORK, origin, "private Emit scan setup")?;
    let mut dimensions = Dimensions {
        functions: length(verified.program.functions.len())?,
        ..Dimensions::default()
    };
    reconcile(&dimensions, upper)?;
    for function in &verified.program.functions {
        debit(work, FUNCTION_WORK, origin, "private Emit scan function")?;
        dimensions.locals = add_length(dimensions.locals, function.locals.len())?;
        dimensions.places = add_length(dimensions.places, function.places.len())?;
        dimensions.parameters = add_length(dimensions.parameters, function.param_count)?;
        dimensions.blocks = add_length(dimensions.blocks, function.blocks.len())?;
        reconcile(&dimensions, upper)?;
        for (block_index, block) in function.blocks.iter().enumerate() {
            debit(work, BLOCK_WORK, origin, "private Emit scan block")?;
            dimensions.statements = add_length(dimensions.statements, block.statements.len())?;
            dimensions.merges = add(dimensions.merges, u64::from(block.merge.is_some()))?;
            reconcile(&dimensions, upper)?;
            let terminator = block.terminator.as_ref().ok_or(Failure::Invariant)?;
            match &terminator.kind {
                TerminatorKind::Return(_) => {}
                TerminatorKind::Goto { target } => {
                    dimensions.edges = add(dimensions.edges, 1)?;
                    dimensions.maybe_cyclic |= target.0 <= block_index;
                }
                TerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    // Duplicate targets still represent two occurrences.
                    dimensions.edges = add(dimensions.edges, 2)?;
                    dimensions.maybe_cyclic |=
                        then_block.0 <= block_index || else_block.0 <= block_index;
                }
                TerminatorKind::Call {
                    args, continuation, ..
                } => {
                    dimensions.calls = add(dimensions.calls, 1)?;
                    dimensions.arguments = add_length(dimensions.arguments, args.len())?;
                    dimensions.edges = add(dimensions.edges, 1)?;
                    dimensions.maybe_cyclic |= continuation.0 <= block_index;
                }
            }
            for statement in &block.statements {
                debit(work, STATEMENT_WORK, origin, "private Emit scan statement")?;
                let (failures, byte_range) = arithmetic_failures(statement);
                dimensions.arithmetic_failures = add(dimensions.arithmetic_failures, failures)?;
                dimensions.has_byte_range_failure |= byte_range;
            }
        }
    }
    Ok(dimensions)
}

// This ledger is deliberately separate from scan work and remains unpaid by
// production: no importer includes it yet. Complete tuples, enclosing results,
// actual iterator types and carriers are measured with size_of. Distinct named
// roles are summed even when sequential; no ABI elision or stack-slot reuse is
// assumed. Inherited owner payloads, WorkMeter internals/test traces, allocator
// internals and arbitrary compiler/std-library stack frames are outside this
// named-carrier model and must not be described as bounded by it.
type ScanInputs<'a> = (&'a VerifiedProgram, Upper, &'a WorkMeter, Span);
type DebitInputs<'a> = (&'a WorkMeter, u64, Span, &'static str);

/// Named per-loop roles, including the complete next result and binding.
/// These are inventory types, not new runtime allocations or fake OIR owners.
#[allow(dead_code)]
struct SliceLoop<T: 'static> {
    vector_borrow: &'static Vec<T>,
    slice_borrow: &'static [T],
    constructor_result: std::slice::Iter<'static, T>,
    held_cursor: std::slice::Iter<'static, T>,
    next_result: Option<&'static T>,
    current: &'static T,
}

#[allow(dead_code)]
struct BlockLoop {
    vector_borrow: &'static Vec<BasicBlock>,
    slice_borrow: &'static [BasicBlock],
    slice_iterator: std::slice::Iter<'static, BasicBlock>,
    constructor_result: std::iter::Enumerate<std::slice::Iter<'static, BasicBlock>>,
    held_cursor: std::iter::Enumerate<std::slice::Iter<'static, BasicBlock>>,
    next_result: Option<(usize, &'static BasicBlock)>,
    current_tuple: (usize, &'static BasicBlock),
    block_index: usize,
    block: &'static BasicBlock,
}

#[allow(dead_code)]
struct TerminatorBindings {
    optional_borrow: &'static Option<Terminator>,
    as_ref_result: Option<&'static Terminator>,
    required_result: Result<&'static Terminator, Failure>,
    terminator: &'static Terminator,
    kind: &'static TerminatorKind,
    // Sum all branch-specific named bindings; do not assume slot reuse.
    goto_target: &'static BlockId,
    branch_then: &'static BlockId,
    branch_else: &'static BlockId,
    call_args: &'static Vec<Operand>,
    call_continuation: &'static BlockId,
}

#[allow(dead_code)]
struct ArithmeticBindings {
    caller_input: &'static Statement,
    callee_input: &'static Statement,
    assignment: &'static Assign,
    value: &'static Rvalue,
    operator: &'static hir::ArithmeticOp,
    returned_counts: (u64, bool),
    caller_counts: (u64, bool),
    failures: u64,
    byte_range: bool,
}

fn byte_add(left: usize, right: usize) -> Result<usize, Failure> {
    left.checked_add(right).ok_or(Failure::Overflow)
}

/// Complete local named carrier/call/result/iterator inventory. This measures
/// this disconnected scan only. A future caller must price its own complete
/// enclosing changed mode, held upper/result and outside ownership carriers,
/// and admit both banks before entry. It must not replace an enclosing enum
/// with a subtotal for only this enum variant's fields.
pub(in crate::frontend::oir) fn named_bytes() -> Result<usize, Failure> {
    let roles = [
        // Caller argument transport and callee inputs.
        size_of::<ScanInputs<'_>>(),
        size_of::<ScanInputs<'_>>(),
        // Default return, constructed accumulator and success payload move.
        size_of::<Dimensions>(),
        size_of::<Dimensions>(),
        size_of::<Dimensions>(),
        // Scan return and caller-held enclosing result; no owner can escape.
        size_of::<Result<Dimensions, Failure>>(),
        size_of::<Result<Dimensions, Failure>>(),
        size_of::<SliceLoop<Function>>(),
        size_of::<BlockLoop>(),
        size_of::<SliceLoop<Statement>>(),
        size_of::<TerminatorBindings>(),
        size_of::<ArithmeticBindings>(),
        // One function-vector len call and six add_length call sites (locals,
        // places, parameters, blocks, statements, arguments). These include
        // complete argument transports and their held results.
        size_of::<(&Vec<Function>,)>(),
        size_of::<usize>(),
        size_of::<Result<u64, Failure>>(),
        size_of::<[(u64, usize); 6]>(),
        size_of::<[Result<u64, Failure>; 6]>(),
        // add_length callee input, length input/result and add input/result.
        size_of::<(u64, usize)>(),
        size_of::<usize>(),
        size_of::<Result<u64, Failure>>(),
        size_of::<(u64, u64)>(),
        size_of::<Result<u64, Failure>>(),
        // length callee input, checked TryFrom transport, fixed mapped return.
        size_of::<usize>(),
        size_of::<Result<u64, std::num::TryFromIntError>>(),
        size_of::<std::num::TryFromIntError>(),
        size_of::<Result<u64, Failure>>(),
        // Six direct add sites: merges, Goto/Branch/Call edge counts, calls,
        // and arithmetic failure count. The callee's checked Option and full
        // returned Result are distinct from these held call results.
        size_of::<[(u64, u64); 6]>(),
        size_of::<[Result<u64, Failure>; 6]>(),
        size_of::<(u64, u64)>(),
        size_of::<Option<u64>>(),
        size_of::<Result<u64, Failure>>(),
        // Reconcile has three call sites plus its own complete input and
        // checked slots sum. Failure transports are fixed discriminants.
        size_of::<[(&Dimensions, Upper); 3]>(),
        size_of::<[Result<(), Failure>; 3]>(),
        size_of::<(&Dimensions, Upper)>(),
        size_of::<(u64, u64)>(),
        size_of::<Result<u64, Failure>>(),
        size_of::<u64>(),
        size_of::<Result<(), Failure>>(),
        // Four paid boundaries; one reusable debit helper frame, with full
        // WorkMeter call/result even though the allocating error is excluded
        // by the immediately preceding original-meter remaining check.
        size_of::<[DebitInputs<'_>; 4]>(),
        size_of::<[Result<(), Failure>; 4]>(),
        size_of::<DebitInputs<'_>>(),
        size_of::<(&WorkMeter,)>(),
        size_of::<(&WorkMeter,)>(),
        size_of::<(u64, u64)>(),
        size_of::<Option<u64>>(),
        size_of::<Result<u64, Failure>>(),
        size_of::<u64>(),
        size_of::<DebitInputs<'_>>(),
        size_of::<Result<(), Box<Diagnostic>>>(),
        size_of::<Box<Diagnostic>>(),
        size_of::<Result<(), Failure>>(),
        // Fixed conversions/comparison expression transports. These complete
        // tuples cover merge presence/value and the three q-update branches.
        size_of::<(&Option<BoolMerge>,)>(),
        size_of::<bool>(),
        size_of::<u64>(),
        size_of::<(usize, usize, bool)>(),
        size_of::<(usize, usize, usize, bool, bool, bool)>(),
        size_of::<(usize, usize, bool)>(),
        // RFC0030's paid per-statement byte-range selector OR transports.
        size_of::<(bool, bool, bool)>(),
        // Eight fixed failure expressions: add/length/byte_add overflow,
        // reconcile mismatch, debit's two Work branches and Invariant mapping,
        // and the scan's missing-terminator invariant. Complete wrappers above
        // are also retained. No Diagnostic payload is allocated by this scan.
        size_of::<[Failure; 8]>(),
        // This inventory's own named sum and byte_add call/return roles.
        size_of::<usize>(),
        size_of::<usize>(),
        size_of::<(usize, usize)>(),
        size_of::<Option<usize>>(),
        size_of::<Result<usize, Failure>>(),
        size_of::<Result<usize, Failure>>(),
        size_of::<Option<usize>>(),
    ];
    // Price the complete concrete array and its move transport, the actual
    // IntoIter carrier, and next-result transport. No field subtotal is used.
    let mut total = byte_add(size_of_val(&roles), size_of_val(&roles))?;
    total = byte_add(total, size_of_val(&roles.into_iter()))?;
    total = byte_add(total, size_of::<Option<usize>>())?;
    for bytes in roles {
        total = byte_add(total, bytes)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::source::SourceFileId;

    const LITERAL: &str = "fn main()->i32{return 0;}";
    const LITERAL_UPPER: Upper = Upper {
        functions: 1,
        blocks: 1,
        slots: 1,
        definitions: 1,
    };
    const RICH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    const RICH_UPPER: Upper = Upper {
        functions: 2,
        blocks: 45,
        slots: 20,
        definitions: 18,
    };

    fn origin() -> Span {
        Span {
            file: SourceFileId(0),
            start: 0,
            end: 0,
        }
    }

    fn literal_dimensions() -> Dimensions {
        Dimensions {
            functions: 1,
            locals: 1,
            blocks: 1,
            statements: 1,
            ..Dimensions::default()
        }
    }

    /// Fixed-data observation arithmetic only, not native-body prepayment.
    fn scan_work(dimensions: Dimensions) -> Result<u64, Failure> {
        let functions = dimensions
            .functions
            .checked_mul(FUNCTION_WORK)
            .ok_or(Failure::Overflow)?;
        let blocks = dimensions
            .blocks
            .checked_mul(BLOCK_WORK)
            .ok_or(Failure::Overflow)?;
        let statements = dimensions
            .statements
            .checked_mul(STATEMENT_WORK)
            .ok_or(Failure::Overflow)?;
        add(add(add(SETUP_WORK, functions)?, blocks)?, statements)
    }

    fn observed(text: &str) -> Dimensions {
        let (verified, _sources) = super::super::tests::verified_with_sources(text);
        let work = WorkMeter::default();
        let dimensions = scan(
            &verified,
            Upper {
                functions: 256,
                blocks: 4_096,
                slots: 8_192,
                definitions: 8_192,
            },
            &work,
            origin(),
        )
        .unwrap();
        assert_eq!(work.used(), scan_work(dimensions).unwrap());
        dimensions
    }

    #[test]
    fn disconnected_paid_emit_scan_exact_and_minus_one() {
        let (verified, _sources) = super::super::tests::verified_with_sources(LITERAL);
        let expected = literal_dimensions();
        let exact = scan_work(expected).unwrap();
        assert_eq!(exact, 4_608);
        let work = WorkMeter::new(exact);
        assert_eq!(
            scan(&verified, LITERAL_UPPER, &work, origin()),
            Ok(expected)
        );
        assert_eq!(work.used(), exact);
        let short = WorkMeter::new(exact - 1);
        assert_eq!(
            scan(&verified, LITERAL_UPPER, &short, origin()),
            Err(Failure::Work)
        );
        assert_eq!(short.used(), exact - STATEMENT_WORK);
    }

    #[test]
    fn disconnected_paid_emit_scan_stops_at_each_unpaid_boundary() {
        let (verified, _sources) = super::super::tests::verified_with_sources(LITERAL);
        for (limit, used) in [
            (0, 0),
            (4_095, 0),
            (4_096, 4_096),
            (4_223, 4_096),
            (4_224, 4_224),
            (4_479, 4_224),
            (4_480, 4_480),
            (4_607, 4_480),
        ] {
            let work = WorkMeter::new(limit);
            assert_eq!(
                scan(&verified, LITERAL_UPPER, &work, origin()),
                Err(Failure::Work)
            );
            assert_eq!(work.used(), used);
        }
    }

    #[test]
    fn disconnected_paid_emit_scan_preserves_preused_original_meter() {
        let (verified, _sources) = super::super::tests::verified_with_sources(LITERAL);
        let prior = 137;
        let exact = scan_work(literal_dimensions()).unwrap();
        let work = WorkMeter::new(prior + exact);
        work.debit(prior, origin(), "already-paid HIR work")
            .unwrap();
        assert_eq!(
            scan(&verified, LITERAL_UPPER, &work, origin()),
            Ok(literal_dimensions())
        );
        assert_eq!(work.used(), prior + exact);

        let short = WorkMeter::new(prior + exact - 1);
        short
            .debit(prior, origin(), "already-paid HIR work")
            .unwrap();
        assert_eq!(
            scan(&verified, LITERAL_UPPER, &short, origin()),
            Err(Failure::Work)
        );
        assert_eq!(short.used(), prior + exact - STATEMENT_WORK);
        assert_eq!(WorkMeter::new(u64::MAX).limit(), 256_000_000);
    }

    #[test]
    fn disconnected_paid_emit_scan_reconciles_each_hir_upper_dimension() {
        let (verified, _sources) = super::super::tests::verified_with_sources(LITERAL);
        for (upper, used) in [
            (
                Upper {
                    functions: 0,
                    ..LITERAL_UPPER
                },
                4_096,
            ),
            (
                Upper {
                    blocks: 0,
                    ..LITERAL_UPPER
                },
                4_224,
            ),
            (
                Upper {
                    slots: 0,
                    ..LITERAL_UPPER
                },
                4_224,
            ),
            (
                Upper {
                    definitions: 0,
                    ..LITERAL_UPPER
                },
                4_480,
            ),
        ] {
            let work = WorkMeter::default();
            assert_eq!(
                scan(&verified, upper, &work, origin()),
                Err(Failure::UpperMismatch)
            );
            assert_eq!(work.used(), used);
        }
    }

    #[test]
    fn disconnected_paid_emit_scan_rich_actual_dimensions() {
        let (verified, _sources) = super::super::tests::verified_with_sources(RICH);
        let expected = Dimensions {
            functions: 2,
            locals: 12,
            places: 1,
            parameters: 1,
            blocks: 9,
            statements: 11,
            merges: 0,
            calls: 2,
            arguments: 2,
            edges: 8,
            arithmetic_failures: 1,
            has_byte_range_failure: false,
            maybe_cyclic: true,
        };
        let exact = scan_work(expected).unwrap();
        assert_eq!(exact, 8_064);
        let work = WorkMeter::new(exact);
        assert_eq!(scan(&verified, RICH_UPPER, &work, origin()), Ok(expected));
        assert_eq!(work.used(), exact);
        let short = WorkMeter::new(exact - 1);
        assert_eq!(
            scan(&verified, RICH_UPPER, &short, origin()),
            Err(Failure::Work)
        );
        assert!(short.used() < exact);
    }

    #[test]
    fn disconnected_paid_emit_scan_arithmetic_and_merge_dimensions() {
        let division = observed("fn main()->i32{return 4/2;}");
        assert_eq!(
            division,
            Dimensions {
                functions: 1,
                locals: 3,
                blocks: 1,
                statements: 3,
                arithmetic_failures: 2,
                ..Dimensions::default()
            }
        );
        let all = observed("fn main()->i32{return -(1+2)-(3*4)/(5%2);}");
        assert_eq!(all.arithmetic_failures, 8);
        assert!(!all.maybe_cyclic);
        let merge = observed("fn main()->bool{return true&&false;}");
        assert_eq!(merge.merges, 1);
        assert_eq!(merge.edges, 3);
        assert_eq!(merge.arithmetic_failures, 0);
        assert!(!merge.maybe_cyclic);
    }

    #[test]
    fn disconnected_paid_emit_scan_backward_edges_are_only_a_work_selector() {
        let loop_dimensions = observed("fn main()->i32{while false{}return 0;}");
        assert_eq!(
            loop_dimensions,
            Dimensions {
                functions: 1,
                locals: 2,
                blocks: 4,
                statements: 2,
                edges: 4,
                maybe_cyclic: true,
                ..Dimensions::default()
            }
        );
        // Nested short circuiting is acyclic, but its later-created inner join
        // jumps to the earlier-created outer join. q must remain conservative.
        let acyclic = observed("fn main()->bool{return true&&(true&&false);}");
        assert_eq!(acyclic.merges, 2);
        assert!(acyclic.maybe_cyclic);
        // Unused functions are still counted, including an unused loop.
        let unused = observed("fn idle()->(){while false{}return;}fn main()->i32{return 0;}");
        assert_eq!(unused.functions, 2);
        assert_eq!(unused.calls, 0);
        assert!(unused.maybe_cyclic);
    }

    #[test]
    fn disconnected_paid_emit_scan_checked_fixed_arithmetic() {
        assert_eq!(add(u64::MAX, 1), Err(Failure::Overflow));
        assert_eq!(add_length(u64::MAX, 1), Err(Failure::Overflow));
        assert_eq!(byte_add(usize::MAX, 1), Err(Failure::Overflow));
        assert_eq!(
            length(usize::MAX),
            u64::try_from(usize::MAX).map_err(|_| Failure::Overflow)
        );
        let largest = Upper {
            functions: u64::MAX,
            blocks: u64::MAX,
            slots: u64::MAX,
            definitions: u64::MAX,
        };
        assert_eq!(
            reconcile(
                &Dimensions {
                    locals: u64::MAX,
                    places: 1,
                    ..Dimensions::default()
                },
                largest
            ),
            Err(Failure::Overflow)
        );
        for dimensions in [
            Dimensions {
                functions: u64::MAX,
                ..Dimensions::default()
            },
            Dimensions {
                blocks: u64::MAX,
                ..Dimensions::default()
            },
            Dimensions {
                statements: u64::MAX,
                ..Dimensions::default()
            },
        ] {
            assert_eq!(scan_work(dimensions), Err(Failure::Overflow));
        }
        let work = WorkMeter::new(8);
        assert_eq!(
            debit(&work, u64::MAX, origin(), "overflow control"),
            Err(Failure::Work)
        );
        assert_eq!(work.used(), 0);
    }

    #[test]
    fn disconnected_paid_emit_scan_named_layout_only() {
        let named = named_bytes().unwrap();
        assert!(named > size_of::<ScanInputs<'_>>() + size_of::<Dimensions>());
        assert!(named < 16 * 1024 * 1024);
        println!(
            "PRIVATE_EMIT_SCAN disconnected_not_yet_integrated paid_outside named={} dimensions={} upper={} failure={} inputs={} result={} functions_cursor={} blocks_cursor={} statements_cursor={} terminator_bindings={} arithmetic_bindings={}",
            named, size_of::<Dimensions>(), size_of::<Upper>(), size_of::<Failure>(),
            size_of::<ScanInputs<'_>>(), size_of::<Result<Dimensions, Failure>>(),
            size_of::<SliceLoop<Function>>(), size_of::<BlockLoop>(),
            size_of::<SliceLoop<Statement>>(), size_of::<TerminatorBindings>(),
            size_of::<ArithmeticBindings>()
        );
    }

    #[test]
    fn u8_emit_scan_successor_exact_and_one_under() {
        let (verified, _) = super::super::tests::verified_with_sources(
            "fn main()->i32{let x=255;let b=x.to_u8_checked();return b.to_i32();}",
        );
        // Two bindings plus literal/read/narrow/read/widen temporaries;
        // seven assignments, one return. Widening adds no diagnostic site.
        let expected = Dimensions {
            functions: 1,
            locals: 7,
            blocks: 1,
            statements: 7,
            arithmetic_failures: 1,
            has_byte_range_failure: true,
            ..Dimensions::default()
        };
        let upper = Upper {
            functions: 1,
            blocks: 1,
            slots: 7,
            definitions: 7,
        };
        let exact = 5_376;
        assert_eq!(scan_work(expected).unwrap(), exact);
        let paid = WorkMeter::new(exact);
        assert_eq!(scan(&verified, upper, &paid, origin()), Ok(expected));
        assert_eq!(paid.used(), exact);
        let short = WorkMeter::new(exact - 1);
        assert_eq!(scan(&verified, upper, &short, origin()), Err(Failure::Work));
        assert_eq!(short.used(), exact - STATEMENT_WORK);
        println!("U8_EMIT_SCAN_SUCCESSOR exact_work={exact} named_bytes={} dimensions_bytes={} failure_bindings_bytes={}", named_bytes().unwrap(), size_of::<Dimensions>(), size_of::<ArithmeticBindings>());
    }
}
