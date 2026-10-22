//! Native resource operations derived from the immutable execution witness.
use super::*;

pub(super) fn contains(plan: &ExecutionPlan<'_>, aggregate: AggregateTy) -> bool {
    plan.witness()
        .declarations()
        .contains_resources(aggregate)
        .expect("verified resource containment")
}

#[derive(Clone, Copy)]
pub(super) enum Action<'a> {
    Validate,
    Move(&'a str),
    Release,
}

/// Records cannot contain enum fields. The enclosing tag-directed emitter
/// handles enum alternatives before entering this static resource-leaf walk.
#[allow(clippy::too_many_arguments)]
pub(super) fn leaves(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    action: Action<'_>,
    kind: Expansion,
    failure: (&Diagnostics, Span),
) {
    if !contains(plan, aggregate) {
        return;
    }
    let declarations = plan.witness().declarations();
    let walk = || {
        declarations
            .resource_leaves(aggregate, |_| Err(DeclarationError::TypeMismatch))
            .expect("verified static resource leaves")
    };
    // Validate every descriptor before moving or releasing any leaf. There is
    // no allocation, byte-work charge, or external effect after this admission.
    for (index, leaf) in walk().enumerate() {
        let leaf = leaf.expect("enum alternatives handled by the enclosing emitter");
        for _ in 0..3 {
            if !out.expand(kind) {
                return;
            }
        }
        let cell = format!("{name}_resource{index}");
        let input = field_pointer(out, &cell, source, leaf.offset);
        writeln!(out, "  %{cell}_status = call i32 @__oxid_bytebuf_valid(ptr {input})\n  %{cell}_valid = icmp eq i32 %{cell}_status, 0\n  br i1 %{cell}_valid, label %{cell}_ok, label %{cell}_error\n{cell}_error:").unwrap();
        emit_failure(out, failure.0, FailureKind::ResourceDescriptor, failure.1);
        writeln!(out, "{cell}_ok:").unwrap();
    }
    if matches!(action, Action::Validate) {
        return;
    }
    for (index, leaf) in walk().enumerate() {
        let leaf = leaf.expect("verified resource leaf");
        let cell = format!("{name}_resource{index}");
        let input = format!("%{cell}_ptr");
        match action {
            Action::Release => {
                writeln!(out, "  call void @__oxid_bytebuf_drop(ptr {input})").unwrap()
            }
            Action::Move(destination) => {
                let output = field_pointer(out, &format!("{cell}_out"), destination, leaf.offset);
                // A descriptor is a resource, not a ScalarLeaf or scalar value.
                // The old bytes become empty while the destination acquires the
                // single obligation. The payload allocation is never copied.
                writeln!(out, "  %{cell}_descriptor = load {{ i64, i32, i32 }}, ptr {input}, align 8\n  store {{ i64, i32, i32 }} %{cell}_descriptor, ptr {output}, align 8\n  store {{ i64, i32, i32 }} zeroinitializer, ptr {input}, align 8").unwrap();
            }
            Action::Validate => unreachable!(),
        }
    }
}

pub(super) fn initialize(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    aggregate: AggregateTy,
    pointer: &str,
) {
    if contains(plan, aggregate) {
        let size = plan
            .witness()
            .declarations()
            .aggregate_layout(aggregate)
            .expect("verified owner layout")
            .size();
        // Empty descriptors and tag zero give uninitialized/moved owner storage
        // a harmless cleanup representation. This adds no source initialization.
        writeln!(
            out,
            "  call void @llvm.memset.p0.i64(ptr align 8 {pointer}, i8 0, i64 {size}, i1 false)"
        )
        .unwrap();
    }
}

pub(super) fn drop_owner(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    pointer: &str,
    failure: (&Diagnostics, Span),
) {
    if contains(plan, aggregate) {
        active_transfer(out, plan, name, aggregate, pointer, None, failure);
    }
}

pub(super) fn drop_frame(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    f: &RawOwnedFunction,
    name: &str,
    failure: (&Diagnostics, Span),
) {
    for (index, owner) in f.owners.iter().enumerate().rev() {
        if out.exceeded {
            return;
        }
        drop_owner(
            out,
            plan,
            &format!("{name}_drop{index}"),
            owner.aggregate(),
            &format!("%o{index}"),
            failure,
        );
    }
}
