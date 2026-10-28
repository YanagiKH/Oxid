//! Native resource operations derived from the immutable execution witness.
use super::*;

fn tag_refusal() -> impl FnMut(usize) -> Result<usize, DeclarationError> {
    |_| Err(DeclarationError::TypeMismatch)
}

#[allow(dead_code)]
struct EmitterWalkCarriers {
    action: Action<'static>,
    enumeration_index: usize,
    descriptor_cells: usize,
    emission_names: [String; 4],
    descriptor_kind: ResourceKind,
    vector_type: hir::Ty,
    vector_element: byte_buffer::VectorElement,
    vector_tag: u32,
    vector_validation_format: FormatCallsiteBacking<3>,
    vector_drop_format: FormatCallsiteBacking<2>,
    vector_writes: [std::fmt::Result; 2],
    enumeration_next: Option<(usize, Result<ResourceLeaf, DeclarationError>)>,
    enumeration_item: (usize, Result<ResourceLeaf, DeclarationError>),
}

pub(super) fn control_bytes(plan: &ExecutionPlan<'_>) -> usize {
    let declarations = plan.witness().declarations();
    let aggregate = AggregateTy::ByteBuf;
    // Use the real enclosing closure capture shape and real zero-sized reader.
    let walk = || declarations.resource_leaves(aggregate, tag_refusal());
    let reader = tag_refusal();
    resource_walk_control_bytes(&reader)
        + enumerated_resource_walk_bytes(&reader)
        + std::mem::size_of_val(&walk)
        + size_of::<EmitterWalkCarriers>()
}

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

/// Only aggregates without enum fields use this static walk. Enum alternatives
/// and records with tagged descendants use their enclosing active transfer.
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
            .resource_leaves(aggregate, tag_refusal())
            .expect("verified static resource leaves")
    };
    // Validate every descriptor before moving or releasing any leaf. There is
    // no allocation, byte-work charge, or external effect after this admission.
    for (index, leaf) in walk().enumerate() {
        let leaf = leaf.expect("enum alternatives handled by the enclosing emitter");
        let descriptor_cells = match leaf.kind {
            ResourceKind::ByteBuf | ResourceKind::Vector(_) | ResourceKind::RecordVector(_) => 3,
            ResourceKind::File => 2,
        };
        for _ in 0..descriptor_cells {
            if !out.expand(kind) {
                return;
            }
        }
        let cell = format!("{name}_resource{index}");
        let input = field_pointer(out, &cell, source, leaf.offset);
        match leaf.kind {
            ResourceKind::ByteBuf => {
                writeln!(
                    out,
                    "  %{cell}_status = call i32 @__oxid_bytebuf_valid(ptr {input})"
                )
                .unwrap();
            }
            ResourceKind::File => {
                writeln!(
                    out,
                    "  %{cell}_status = call i32 @__oxid_file_valid(ptr {input})"
                )
                .unwrap();
            }
            ResourceKind::Vector(_) | ResourceKind::RecordVector(_) => {
                let tag = match leaf.kind {
                    ResourceKind::Vector(ty) => super::super::vector_type::element(ty).tag(),
                    ResourceKind::RecordVector(id) => 0x8000_0000 | id,
                    _ => unreachable!(),
                };
                writeln!(
                    out,
                    "  %{cell}_status = call i32 @__oxid_vector_valid(ptr {input}, i32 {tag})"
                )
                .unwrap();
            }
        }
        writeln!(out, "  %{cell}_valid = icmp eq i32 %{cell}_status, 0\n  br i1 %{cell}_valid, label %{cell}_ok, label %{cell}_error\n{cell}_error:").unwrap();
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
            Action::Release => match leaf.kind {
                ResourceKind::ByteBuf => {
                    writeln!(out, "  call void @__oxid_bytebuf_drop(ptr {input})").unwrap();
                }
                ResourceKind::File => {
                    writeln!(out, "  call void @__oxid_file_drop(ptr {input})").unwrap();
                }
                ResourceKind::Vector(_) | ResourceKind::RecordVector(_) => {
                    let tag = match leaf.kind {
                        ResourceKind::Vector(ty) => super::super::vector_type::element(ty).tag(),
                        ResourceKind::RecordVector(id) => 0x8000_0000 | id,
                        _ => unreachable!(),
                    };
                    writeln!(
                        out,
                        "  call void @__oxid_vector_drop(ptr {input}, i32 {tag})"
                    )
                    .unwrap();
                }
            },
            Action::Move(destination) => {
                let output = field_pointer(out, &format!("{cell}_out"), destination, leaf.offset);
                // A descriptor is a resource, not a ScalarLeaf or scalar value.
                // The old bytes become empty while the destination acquires the
                // single obligation. The payload allocation is never copied.
                let carrier = match leaf.kind {
                    ResourceKind::ByteBuf
                    | ResourceKind::Vector(_)
                    | ResourceKind::RecordVector(_) => "{ i64, i32, i32 }",
                    ResourceKind::File => "{ i64, i64 }",
                };
                writeln!(out, "  %{cell}_descriptor = load {carrier}, ptr {input}, align 8\n  store {carrier} %{cell}_descriptor, ptr {output}, align 8\n  store {carrier} zeroinitializer, ptr {input}, align 8").unwrap();
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
