//! Test-only retained-capacity observations for successful canonical checks.
//! These exclude HIR, owner headers, allocator metadata and allocator transients;
//! they are neither admission limits nor error-path or RSS peaks.
use super::*;
use std::{cell::Cell, marker::PhantomData, mem::size_of, rc::Rc};

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct Snapshot {
    pub frame_bytes: usize,
    pub frame_alignment: usize,
    pub frames_vec_bytes: usize,
    pub frames_capacity_max: usize,
    pub expressions_capacity_max: usize,
    pub locals_source_capacity_max: usize,
    pub locals_capacity_max: usize,
    pub flows_source_capacity_max: usize,
    pub flows_capacity_max: usize,
    pub bodies_capacity_max: usize,
    /// Maximum successful-body envelope: retained empty frames, expressions,
    /// both conversion sources and both destinations, conservatively summed.
    /// Sources/destinations need not coexist and buffer reuse is not assumed.
    pub body_payload_envelope_bytes: usize,
}

thread_local! {
    static ACTIVE: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

/// Thread-bound activation. Drop resets observations even during unwinding.
#[must_use]
pub(in crate::frontend) struct Guard(PhantomData<Rc<()>>);

pub(in crate::frontend) fn begin() -> Guard {
    ACTIVE.with(|active| {
        assert!(
            active.get().is_none(),
            "typecheck measurement already active"
        );
        active.set(Some(Snapshot::default()));
    });
    Guard(PhantomData)
}

impl Guard {
    pub(in crate::frontend) fn finish(self) -> Snapshot {
        ACTIVE.with(|active| active.take().expect("active typecheck measurement"))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(None));
    }
}

fn update(f: impl FnOnce(&mut Snapshot)) {
    ACTIVE.with(|active| {
        if let Some(mut snapshot) = active.get() {
            f(&mut snapshot);
            active.set(Some(snapshot));
        }
    });
}

pub(super) fn frame_layout(bytes: usize, alignment: usize, vec_bytes: usize) {
    update(|s| {
        s.frame_bytes = bytes;
        s.frame_alignment = alignment;
        s.frames_vec_bytes = vec_bytes;
    });
}

pub(super) fn frames_capacity(capacity: usize) {
    update(|s| s.frames_capacity_max = s.frames_capacity_max.max(capacity));
}

pub(super) fn bodies_capacity(capacity: usize) {
    update(|s| s.bodies_capacity_max = s.bodies_capacity_max.max(capacity));
}

pub(super) fn body_finished(
    frames: usize,
    expressions: usize,
    locals_source: usize,
    locals: usize,
    flows_source: usize,
    flows: usize,
) {
    update(|s| {
        s.frames_capacity_max = s.frames_capacity_max.max(frames);
        s.expressions_capacity_max = s.expressions_capacity_max.max(expressions);
        s.locals_source_capacity_max = s.locals_source_capacity_max.max(locals_source);
        s.locals_capacity_max = s.locals_capacity_max.max(locals);
        s.flows_source_capacity_max = s.flows_source_capacity_max.max(flows_source);
        s.flows_capacity_max = s.flows_capacity_max.max(flows);
        let bytes = [
            frames.checked_mul(s.frame_bytes),
            expressions.checked_mul(size_of::<Ty>()),
            locals_source.checked_mul(size_of::<Option<Ty>>()),
            locals.checked_mul(size_of::<Ty>()),
            flows_source.checked_mul(size_of::<Option<FlowSummary>>()),
            flows.checked_mul(size_of::<FlowSummary>()),
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes?))
        .expect("typecheck observation byte sum");
        s.body_payload_envelope_bytes = s.body_payload_envelope_bytes.max(bytes);
    });
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // All fields are emitted by the immutable Debug measurement report.
pub(in crate::frontend) struct Layout {
    pub typed_program: usize,
    pub typed_body: usize,
    pub flow: usize,
    pub option_flow: usize,
    pub ty: usize,
    pub option_ty: usize,
    pub bodies_vec: usize,
    pub types_vec: usize,
    pub option_types_vec: usize,
    pub flows_vec: usize,
    pub option_flows_vec: usize,
    pub check_result: usize,
    pub body_result: usize,
}

pub(in crate::frontend) fn layout() -> Layout {
    Layout {
        typed_program: size_of::<TypedProgram>(),
        typed_body: size_of::<TypedBody>(),
        flow: size_of::<FlowSummary>(),
        option_flow: size_of::<Option<FlowSummary>>(),
        ty: size_of::<Ty>(),
        option_ty: size_of::<Option<Ty>>(),
        bodies_vec: size_of::<Vec<TypedBody>>(),
        types_vec: size_of::<Vec<Ty>>(),
        option_types_vec: size_of::<Vec<Option<Ty>>>(),
        flows_vec: size_of::<Vec<FlowSummary>>(),
        option_flows_vec: size_of::<Vec<Option<FlowSummary>>>(),
        check_result: size_of::<Result<TypedProgram, Vec<Diagnostic>>>(),
        body_result: size_of::<Result<TypedBody, Box<Diagnostic>>>(),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct RetainedPayload {
    pub bodies_capacity: usize,
    pub expressions_capacity: usize,
    pub locals_capacity: usize,
    pub flows_capacity: usize,
    pub bytes: usize,
}

/// Does not include the still-owned HIR or the enclosing TypedProgram header.
/// TypedBody's embedded Vec headers are counted in the bodies allocation once.
pub(in crate::frontend) fn retained_payload(typed: &TypedProgram) -> Option<RetainedPayload> {
    let mut facts = RetainedPayload {
        bodies_capacity: typed.bodies.capacity(),
        ..RetainedPayload::default()
    };
    for body in &typed.bodies {
        facts.expressions_capacity = facts
            .expressions_capacity
            .checked_add(body.expressions.capacity())?;
        facts.locals_capacity = facts.locals_capacity.checked_add(body.locals.capacity())?;
        facts.flows_capacity = facts
            .flows_capacity
            .checked_add(body.block_flows.capacity())?;
    }
    facts.bytes = facts
        .bodies_capacity
        .checked_mul(size_of::<TypedBody>())?
        .checked_add(facts.expressions_capacity.checked_mul(size_of::<Ty>())?)?
        .checked_add(facts.locals_capacity.checked_mul(size_of::<Ty>())?)?
        .checked_add(facts.flows_capacity.checked_mul(size_of::<FlowSummary>())?)?;
    Some(facts)
}

#[test]
fn activation_rejects_nesting_and_resets_on_finish_and_unwind() {
    let guard = begin();
    frames_capacity(7);
    assert!(std::panic::catch_unwind(begin).is_err());
    assert_eq!(guard.finish().frames_capacity_max, 7);
    assert_eq!(begin().finish().frames_capacity_max, 0);
    assert!(std::panic::catch_unwind(|| {
        let _guard = begin();
        frames_capacity(11);
        panic!("exercise measurement unwind");
    })
    .is_err());
    assert_eq!(begin().finish().frames_capacity_max, 0);
}
