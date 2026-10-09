//! Test-only success-path capacities, not admission bounds or allocator/RSS peaks.
//! Final raw payload plus the sum of scratch-family maxima is conservative for
//! successful lowering: raw owners/capacities are retained until return. It does
//! not cover a failed prefix, owner headers or the still-live TypedProgram/HIR.
use super::*;
use std::{cell::Cell, marker::PhantomData, mem::size_of, rc::Rc};

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct Snapshot {
    pub local_map_capacity_max: usize,
    pub expression_map_capacity_max: usize,
    pub loop_targets_capacity_max: usize,
    pub active_loops_capacity_max: usize,
    pub frames_capacity_max: usize,
    pub expression_frames_capacity_max: usize,
    /// The sum of per-family maximum payloads; they need not coincide.
    pub scratch_payload_envelope_bytes: usize,
}

thread_local! {
    static ACTIVE: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

/// No heap allocation; the marker prevents moving activation to another thread.
#[must_use]
pub(in crate::frontend) struct Guard(PhantomData<Rc<()>>);

pub(in crate::frontend) fn begin() -> Guard {
    ACTIVE.with(|active| {
        assert!(active.get().is_none(), "lower measurement already active");
        active.set(Some(Snapshot::default()));
    });
    Guard(PhantomData)
}

impl Guard {
    pub(in crate::frontend) fn finish(self) -> Snapshot {
        let mut snapshot = ACTIVE.with(|active| active.take().expect("active lower measurement"));
        snapshot.scratch_payload_envelope_bytes = [
            snapshot
                .local_map_capacity_max
                .checked_mul(size_of::<BindingLocation>()),
            snapshot
                .expression_map_capacity_max
                .checked_mul(size_of::<LocalId>()),
            snapshot
                .loop_targets_capacity_max
                .checked_mul(size_of::<Option<LoopTargets>>()),
            snapshot
                .active_loops_capacity_max
                .checked_mul(size_of::<hir::LoopId>()),
            snapshot.frames_capacity_max.checked_mul(size_of::<Frame>()),
            snapshot
                .expression_frames_capacity_max
                .checked_mul(size_of::<ExprFrame>()),
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes?))
        .expect("lower observation byte sum");
        snapshot
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(None));
    }
}

pub(super) fn expression_frames_capacity(capacity: usize) {
    ACTIVE.with(|active| {
        if let Some(mut s) = active.get() {
            s.expression_frames_capacity_max = s.expression_frames_capacity_max.max(capacity);
            active.set(Some(s));
        }
    });
}

pub(super) fn body_scratch(
    locals: usize,
    expressions: usize,
    loops: usize,
    active_loops: usize,
    frames: usize,
) {
    ACTIVE.with(|active| {
        if let Some(mut s) = active.get() {
            s.local_map_capacity_max = s.local_map_capacity_max.max(locals);
            s.expression_map_capacity_max = s.expression_map_capacity_max.max(expressions);
            s.loop_targets_capacity_max = s.loop_targets_capacity_max.max(loops);
            s.active_loops_capacity_max = s.active_loops_capacity_max.max(active_loops);
            s.frames_capacity_max = s.frames_capacity_max.max(frames);
            active.set(Some(s));
        }
    });
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // All fields are emitted by the immutable Debug measurement report.
pub(in crate::frontend) struct Layout {
    pub frame: usize,
    pub expression_frame: usize,
    pub binding_location: usize,
    pub loop_targets: usize,
    pub option_loop_targets: usize,
    pub hir_loop_id: usize,
    pub local_id: usize,
    pub block_builder: usize,
    pub program: usize,
    pub function: usize,
    pub local_decl: usize,
    pub place_decl: usize,
    pub basic_block: usize,
    pub statement: usize,
    pub operand: usize,
    pub local_map_vec: usize,
    pub expression_map_vec: usize,
    pub loop_targets_vec: usize,
    pub active_loops_vec: usize,
    pub frames_vec: usize,
    pub expression_frames_vec: usize,
    pub functions_vec: usize,
    pub locals_vec: usize,
    pub places_vec: usize,
    pub blocks_vec: usize,
    pub statements_vec: usize,
    pub operands_vec: usize,
    pub lower_result: usize,
    pub builder_result: usize,
    pub unit_result: usize,
}

pub(in crate::frontend) fn layout() -> Layout {
    Layout {
        frame: size_of::<Frame>(),
        expression_frame: size_of::<ExprFrame>(),
        binding_location: size_of::<BindingLocation>(),
        loop_targets: size_of::<LoopTargets>(),
        option_loop_targets: size_of::<Option<LoopTargets>>(),
        hir_loop_id: size_of::<hir::LoopId>(),
        local_id: size_of::<LocalId>(),
        block_builder: size_of::<BlockBuilder>(),
        program: size_of::<Program>(),
        function: size_of::<Function>(),
        local_decl: size_of::<LocalDecl>(),
        place_decl: size_of::<PlaceDecl>(),
        basic_block: size_of::<BasicBlock>(),
        statement: size_of::<Statement>(),
        operand: size_of::<Operand>(),
        local_map_vec: size_of::<Vec<BindingLocation>>(),
        expression_map_vec: size_of::<Vec<LocalId>>(),
        loop_targets_vec: size_of::<Vec<Option<LoopTargets>>>(),
        active_loops_vec: size_of::<Vec<hir::LoopId>>(),
        frames_vec: size_of::<Vec<Frame>>(),
        expression_frames_vec: size_of::<Vec<ExprFrame>>(),
        functions_vec: size_of::<Vec<Function>>(),
        locals_vec: size_of::<Vec<LocalDecl>>(),
        places_vec: size_of::<Vec<PlaceDecl>>(),
        blocks_vec: size_of::<Vec<BasicBlock>>(),
        statements_vec: size_of::<Vec<Statement>>(),
        operands_vec: size_of::<Vec<Operand>>(),
        lower_result: size_of::<Result<Program, OirFailure>>(),
        builder_result: size_of::<Result<Vec<BasicBlock>, OirFailure>>(),
        unit_result: size_of::<Result<(), OirFailure>>(),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct RetainedPayload {
    pub functions_capacity: usize,
    pub locals_capacity: usize,
    pub places_capacity: usize,
    pub blocks_capacity: usize,
    pub statements_capacity: usize,
    pub call_arguments_capacity: usize,
    pub bytes: usize,
}

/// Owner headers embedded in the functions/blocks allocations are counted once.
/// Inline merges add no allocation. The outer Program header is caller-owned.
pub(in crate::frontend::oir) fn retained_payload(raw: &Program) -> Option<RetainedPayload> {
    let mut facts = RetainedPayload {
        functions_capacity: raw.functions.capacity(),
        ..RetainedPayload::default()
    };
    for function in &raw.functions {
        facts.locals_capacity = facts
            .locals_capacity
            .checked_add(function.locals.capacity())?;
        facts.places_capacity = facts
            .places_capacity
            .checked_add(function.places.capacity())?;
        facts.blocks_capacity = facts
            .blocks_capacity
            .checked_add(function.blocks.capacity())?;
        for block in &function.blocks {
            facts.statements_capacity = facts
                .statements_capacity
                .checked_add(block.statements.capacity())?;
            if let Some(Terminator {
                kind: TerminatorKind::Call { args, .. },
                ..
            }) = &block.terminator
            {
                facts.call_arguments_capacity =
                    facts.call_arguments_capacity.checked_add(args.capacity())?;
            }
        }
    }
    facts.bytes = facts
        .functions_capacity
        .checked_mul(size_of::<Function>())?
        .checked_add(facts.locals_capacity.checked_mul(size_of::<LocalDecl>())?)?
        .checked_add(facts.places_capacity.checked_mul(size_of::<PlaceDecl>())?)?
        .checked_add(facts.blocks_capacity.checked_mul(size_of::<BasicBlock>())?)?
        .checked_add(
            facts
                .statements_capacity
                .checked_mul(size_of::<Statement>())?,
        )?
        .checked_add(
            facts
                .call_arguments_capacity
                .checked_mul(size_of::<Operand>())?,
        )?;
    Some(facts)
}

#[test]
fn activation_rejects_nesting_and_resets_on_finish_and_unwind() {
    let guard = begin();
    expression_frames_capacity(7);
    assert!(std::panic::catch_unwind(begin).is_err());
    assert_eq!(guard.finish().expression_frames_capacity_max, 7);
    assert_eq!(begin().finish().expression_frames_capacity_max, 0);
    assert!(std::panic::catch_unwind(|| {
        let _guard = begin();
        expression_frames_capacity(11);
        panic!("exercise measurement unwind");
    })
    .is_err());
    assert_eq!(begin().finish().expression_frames_capacity_max, 0);
}

#[test]
fn u8_lower_carrier_size_and_alignment_measurements() {
    println!(
        "u8_lower_carrier frame size={} align={}",
        size_of::<Frame>(),
        std::mem::align_of::<Frame>()
    );
    println!(
        "u8_lower_carrier expression_frame size={} align={}",
        size_of::<ExprFrame>(),
        std::mem::align_of::<ExprFrame>()
    );
    println!(
        "u8_lower_carrier binding_location size={} align={}",
        size_of::<BindingLocation>(),
        std::mem::align_of::<BindingLocation>()
    );
    println!(
        "u8_lower_carrier loop_targets size={} align={}",
        size_of::<LoopTargets>(),
        std::mem::align_of::<LoopTargets>()
    );
    println!(
        "u8_lower_carrier option_loop_targets size={} align={}",
        size_of::<Option<LoopTargets>>(),
        std::mem::align_of::<Option<LoopTargets>>()
    );
    println!(
        "u8_lower_carrier hir_loop_id size={} align={}",
        size_of::<hir::LoopId>(),
        std::mem::align_of::<hir::LoopId>()
    );
    println!(
        "u8_lower_carrier local_id size={} align={}",
        size_of::<LocalId>(),
        std::mem::align_of::<LocalId>()
    );
    println!(
        "u8_lower_carrier block_builder size={} align={}",
        size_of::<BlockBuilder>(),
        std::mem::align_of::<BlockBuilder>()
    );
    println!(
        "u8_lower_carrier program size={} align={}",
        size_of::<Program>(),
        std::mem::align_of::<Program>()
    );
    println!(
        "u8_lower_carrier function size={} align={}",
        size_of::<Function>(),
        std::mem::align_of::<Function>()
    );
    println!(
        "u8_lower_carrier local_decl size={} align={}",
        size_of::<LocalDecl>(),
        std::mem::align_of::<LocalDecl>()
    );
    println!(
        "u8_lower_carrier place_decl size={} align={}",
        size_of::<PlaceDecl>(),
        std::mem::align_of::<PlaceDecl>()
    );
    println!(
        "u8_lower_carrier basic_block size={} align={}",
        size_of::<BasicBlock>(),
        std::mem::align_of::<BasicBlock>()
    );
    println!(
        "u8_lower_carrier statement size={} align={}",
        size_of::<Statement>(),
        std::mem::align_of::<Statement>()
    );
    println!(
        "u8_lower_carrier operand size={} align={}",
        size_of::<Operand>(),
        std::mem::align_of::<Operand>()
    );
    println!(
        "u8_lower_carrier local_map_vec size={} align={}",
        size_of::<Vec<BindingLocation>>(),
        std::mem::align_of::<Vec<BindingLocation>>()
    );
    println!(
        "u8_lower_carrier expression_map_vec size={} align={}",
        size_of::<Vec<LocalId>>(),
        std::mem::align_of::<Vec<LocalId>>()
    );
    println!(
        "u8_lower_carrier loop_targets_vec size={} align={}",
        size_of::<Vec<Option<LoopTargets>>>(),
        std::mem::align_of::<Vec<Option<LoopTargets>>>()
    );
    println!(
        "u8_lower_carrier active_loops_vec size={} align={}",
        size_of::<Vec<hir::LoopId>>(),
        std::mem::align_of::<Vec<hir::LoopId>>()
    );
    println!(
        "u8_lower_carrier frames_vec size={} align={}",
        size_of::<Vec<Frame>>(),
        std::mem::align_of::<Vec<Frame>>()
    );
    println!(
        "u8_lower_carrier expression_frames_vec size={} align={}",
        size_of::<Vec<ExprFrame>>(),
        std::mem::align_of::<Vec<ExprFrame>>()
    );
    println!(
        "u8_lower_carrier functions_vec size={} align={}",
        size_of::<Vec<Function>>(),
        std::mem::align_of::<Vec<Function>>()
    );
    println!(
        "u8_lower_carrier locals_vec size={} align={}",
        size_of::<Vec<LocalDecl>>(),
        std::mem::align_of::<Vec<LocalDecl>>()
    );
    println!(
        "u8_lower_carrier places_vec size={} align={}",
        size_of::<Vec<PlaceDecl>>(),
        std::mem::align_of::<Vec<PlaceDecl>>()
    );
    println!(
        "u8_lower_carrier blocks_vec size={} align={}",
        size_of::<Vec<BasicBlock>>(),
        std::mem::align_of::<Vec<BasicBlock>>()
    );
    println!(
        "u8_lower_carrier statements_vec size={} align={}",
        size_of::<Vec<Statement>>(),
        std::mem::align_of::<Vec<Statement>>()
    );
    println!(
        "u8_lower_carrier operands_vec size={} align={}",
        size_of::<Vec<Operand>>(),
        std::mem::align_of::<Vec<Operand>>()
    );
    println!(
        "u8_lower_carrier lower_result size={} align={}",
        size_of::<Result<Program, OirFailure>>(),
        std::mem::align_of::<Result<Program, OirFailure>>()
    );
    println!(
        "u8_lower_carrier builder_result size={} align={}",
        size_of::<Result<Vec<BasicBlock>, OirFailure>>(),
        std::mem::align_of::<Result<Vec<BasicBlock>, OirFailure>>()
    );
    println!(
        "u8_lower_carrier unit_result size={} align={}",
        size_of::<Result<(), OirFailure>>(),
        std::mem::align_of::<Result<(), OirFailure>>()
    );
}
