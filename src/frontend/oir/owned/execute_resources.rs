//! Built-in resource ownership inside the reference consumer. All layouts
//! originate in checked declarations; no scalar can act as an allocation ID.
use super::*;
use crate::frontend::oir::owned::owned_record_codec::{bounds::CheckedElementView, Element};

/// The legacy declaration-only input and the real exact-witness scope share
/// one active validation/copy/release body. No trait object or table is stored.
pub(super) trait ResourceQueries<'facade> {
    fn declarations(&self) -> &'facade Declarations;
    fn leaves<F>(
        &self,
        aggregate: AggregateTy,
        reader: F,
    ) -> std::result::Result<ResourceLeaves<'facade, F>, DeclarationError>
    where
        F: FnMut(usize) -> std::result::Result<usize, DeclarationError>;
    fn validate_owned(
        &mut self,
        heap: &byte_buffer::Heap,
        record: u32,
        bytes: &[u8],
        span: Span,
    ) -> Result<()>;
    fn release_owned(
        &mut self,
        heap: &mut byte_buffer::Heap,
        record: u32,
        bytes: &[u8],
        span: Span,
    ) -> Result<()>;
}

impl<'a> ResourceQueries<'a> for &'a Declarations {
    fn declarations(&self) -> &'a Declarations {
        self
    }
    fn leaves<F>(
        &self,
        aggregate: AggregateTy,
        reader: F,
    ) -> std::result::Result<ResourceLeaves<'a, F>, DeclarationError>
    where
        F: FnMut(usize) -> std::result::Result<usize, DeclarationError>,
    {
        self.resource_leaves(aggregate, reader)
    }
    fn validate_owned(
        &mut self,
        _: &byte_buffer::Heap,
        _: u32,
        _: &[u8],
        span: Span,
    ) -> Result<()> {
        Err(bad(
            "owned vector resource requires its complete consumer",
            span,
        ))
    }
    fn release_owned(
        &mut self,
        _: &mut byte_buffer::Heap,
        _: u32,
        _: &[u8],
        span: Span,
    ) -> Result<()> {
        Err(bad(
            "owned vector resource requires its complete consumer",
            span,
        ))
    }
}

pub(super) struct RuntimeResourceScope<'a, W: PlanWitness> {
    witness: &'a W,
    fuel: Option<&'a mut usize>,
}
impl<'a, W: PlanWitness> RuntimeResourceScope<'a, W> {
    pub(super) fn paid(witness: &'a W, fuel: &'a mut usize) -> Self {
        Self {
            witness,
            fuel: Some(fuel),
        }
    }
    pub(super) fn maintenance(witness: &'a W) -> Self {
        Self {
            witness,
            fuel: None,
        }
    }
    fn element(&self, record: u32, span: Span) -> Result<Element<'a>> {
        let root = OwnedVectorRootSlot::from_raw(record)
            .ok_or_else(|| bad("owned vector resource domain", span))?
            .root();
        self.witness
            .elements()
            .ok_or_else(|| bad("owned vector resource witness", span))?
            .element_root(root)
            .map_err(|_| bad("owned vector resource element", span))
    }
}
impl<'a, W: PlanWitness> ResourceQueries<'a> for RuntimeResourceScope<'a, W> {
    fn declarations(&self) -> &'a Declarations {
        self.witness.declarations()
    }
    fn leaves<F>(
        &self,
        aggregate: AggregateTy,
        reader: F,
    ) -> std::result::Result<ResourceLeaves<'a, F>, DeclarationError>
    where
        F: FnMut(usize) -> std::result::Result<usize, DeclarationError>,
    {
        self.witness.resource_leaves(aggregate, reader)
    }
    fn validate_owned(
        &mut self,
        heap: &byte_buffer::Heap,
        record: u32,
        bytes: &[u8],
        span: Span,
    ) -> Result<()> {
        let element = self.element(record, span)?;
        let value = byte_buffer::OwnedVectorDescriptor::decode(bytes)
            .map_err(|error| experimental::heap_failure(error, span))?;
        if let Some(fuel) = self.fuel.as_deref_mut() {
            let hosted = *fuel == HOSTED_FUEL;
            heap.owned_vector_validate(&value, element, fuel)
                .map_err(|error| experimental::heap_failure(error, span))?;
            if hosted {
                *fuel = HOSTED_FUEL;
            }
            Ok(())
        } else {
            // Normal release maintenance is separate from user-fueled moves.
            heap.owned_vector_descriptor(bytes, element)
                .map(|_| ())
                .map_err(|error| experimental::heap_failure(error, span))
        }
    }
    fn release_owned(
        &mut self,
        heap: &mut byte_buffer::Heap,
        record: u32,
        bytes: &[u8],
        span: Span,
    ) -> Result<()> {
        let element = self.element(record, span)?;
        let mut value = byte_buffer::OwnedVectorDescriptor::decode(bytes)
            .map_err(|error| experimental::heap_failure(error, span))?;
        heap.owned_vector_release(&mut value, element)
            .map_err(|error| experimental::heap_failure(error, span))
    }
}

struct OwnedResourceCarriers<'a, W: PlanWitness> {
    witness: [&'a W; 2],
    scope: RuntimeResourceScope<'a, W>,
    scope_return: RuntimeResourceScope<'a, W>,
    scope_receiver: &'a mut RuntimeResourceScope<'a, W>,
    fuel: &'a mut usize,
    fuel_option: Option<&'a mut usize>,
    view: Option<CheckedElementView<'a>>,
    element: Element<'a>,
    element_return: Result<Element<'a>>,
    descriptor: byte_buffer::OwnedVectorDescriptor,
    decoded_return: std::result::Result<byte_buffer::OwnedVectorDescriptor, byte_buffer::Failure>,
    mapped_return: Result<byte_buffer::OwnedVectorDescriptor>,
    validation_return: std::result::Result<(), byte_buffer::Failure>,
    mapped_validation_return: Result<()>,
    caller_validation_return: Result<()>,
    maintenance_descriptor_return:
        std::result::Result<byte_buffer::OwnedVectorDescriptor, byte_buffer::Failure>,
    selected_failure: byte_buffer::Failure,
    selected_root: OwnedVectorRoot,
    selected_slot: OwnedVectorRootSlot,
    slot_return: Option<OwnedVectorRootSlot>,
    bytes: [&'a [u8]; 2],
    declaration_return: &'a Declarations,
    hosted: bool,
    transfer_keys: [OwnerKey; 2],
    admitted_replacement: Option<(std::ops::Range<usize>, AggregateTy, std::ops::Range<usize>)>,
}
pub(super) fn owned_control_bytes<W: PlanWitness>() -> usize {
    size_of::<OwnedResourceCarriers<'_, W>>()
        + super::super::owned_record_codec::bounds::element_control_bytes()
}

fn vector_type_error_mapper(
    span: &Span,
) -> impl FnOnce(byte_buffer::Failure) -> OwnedRunFailure + '_ {
    move |_| bad("record vector type", *span)
}

fn tag_reader(
    bytes: &[u8],
) -> impl FnMut(usize) -> std::result::Result<usize, DeclarationError> + '_ {
    move |offset| {
        let end = offset
            .checked_add(4)
            .ok_or(DeclarationError::LayoutOverflow)?;
        let tag = bytes
            .get(offset..end)
            .ok_or(DeclarationError::LayoutOverflow)?;
        Ok(u32::from_le_bytes(tag.try_into().unwrap()) as usize)
    }
}

#[allow(dead_code)]
struct ReferenceWalkCarriers {
    declaration_argument: &'static Declarations,
    aggregate_argument: AggregateTy,
    bytes_arguments: [&'static [u8]; 2],
    extent_returns: [Result<std::ops::Range<usize>>; 2],
    selected_extents: [std::ops::Range<usize>; 2],
    descriptor: byte_buffer::Descriptor,
    descriptor_return: std::result::Result<byte_buffer::Descriptor, byte_buffer::Failure>,
    file_descriptor: byte_buffer::FileDescriptor,
    file_descriptor_return: std::result::Result<byte_buffer::FileDescriptor, byte_buffer::Failure>,
    vector_type: super::super::vector_type::VectorTy,
    vector_element_return:
        std::result::Result<byte_buffer::CheckedVectorElement<'static>, byte_buffer::Failure>,
    mapped_vector_element_return: Result<byte_buffer::CheckedVectorElement<'static>>,
    vector_element: byte_buffer::CheckedVectorElement<'static>,
    vector_descriptor: byte_buffer::VectorDescriptor,
    vector_descriptor_return:
        std::result::Result<byte_buffer::VectorDescriptor, byte_buffer::Failure>,
    vector_validation_return: std::result::Result<(), byte_buffer::Failure>,
    mapped_descriptor_return: Result<byte_buffer::Descriptor>,
    mapped_file_descriptor_return: Result<byte_buffer::FileDescriptor>,
    mapped_vector_descriptor_return: Result<byte_buffer::VectorDescriptor>,
    mapped_leaf_result: Result<ResourceLeaf>,
    mapped_release_result: Result<()>,
    mapped_file_close_result: Result<std::result::Result<(), i32>>,
    tag_reader_slice_option: Option<&'static [u8]>,
    tag_reader_checked_end: Option<usize>,
    tag_reader_end_return: std::result::Result<usize, DeclarationError>,
    tag_reader_end: usize,
    tag_reader_slice_return: std::result::Result<&'static [u8], DeclarationError>,
    tag_reader_slice: &'static [u8],
    tag_array_return: std::result::Result<[u8; 4], std::array::TryFromSliceError>,
    tag_array: [u8; 4],
    decoded_tag: u32,
}

fn mapped_factory_bytes<F>(_: &F) -> usize {
    size_of::<Result<ResourceLeaves<'static, F>>>()
}

pub(super) fn control_bytes() -> usize {
    let reader = tag_reader(&[]);
    // Select the actual mapper's closure type without invoking it. This const
    // is only a borrowed layout input and cannot grant runtime source authority.
    const LAYOUT_SPAN: Span = Span {
        file: crate::frontend::source::SourceFileId(0),
        start: 0,
        end: 0,
    };
    resource_walk_control_bytes(&reader)
        + mapped_factory_bytes(&reader)
        + size_of::<ReferenceWalkCarriers>()
        // Preflight and release select elements sequentially; they share this
        // one nonrecursive caller bank, independently of vector operation banks.
        + std::mem::size_of_val(&vector_type_error_mapper(&LAYOUT_SPAN))
}

fn leaves<'a>(
    declarations: &'a Declarations,
    aggregate: AggregateTy,
    bytes: &'a [u8],
) -> std::result::Result<
    ResourceLeaves<'a, impl FnMut(usize) -> std::result::Result<usize, DeclarationError> + 'a>,
    DeclarationError,
> {
    declarations.resource_leaves(aggregate, tag_reader(bytes))
}

fn extent(offset: usize, length: usize, span: Span) -> Result<std::ops::Range<usize>> {
    let end = offset
        .checked_add(16)
        .filter(|end| *end <= length)
        .ok_or_else(|| bad("resource extent", span))?;
    Ok(offset..end)
}

pub(super) fn preflight<'a, Q: ResourceQueries<'a>>(
    mut queries: Q,
    heap: &byte_buffer::Heap,
    aggregate: AggregateTy,
    source: &[u8],
    destination_length: usize,
    span: Span,
) -> Result<()> {
    preflight_impl(
        &mut queries,
        heap,
        aggregate,
        source,
        destination_length,
        span,
    )
}

fn preflight_impl<'a, Q: ResourceQueries<'a>>(
    queries: &mut Q,
    heap: &byte_buffer::Heap,
    aggregate: AggregateTy,
    source: &[u8],
    destination_length: usize,
    span: Span,
) -> Result<()> {
    let declarations = queries.declarations();
    for leaf in queries
        .leaves(aggregate, tag_reader(source))
        .map_err(|_| bad("resource layout", span))?
    {
        let leaf = leaf.map_err(|_| bad("resource active alternative", span))?;
        let source_extent = extent(leaf.offset, source.len(), span)?;
        extent(leaf.offset, destination_length, span)?;
        match leaf.kind {
            ResourceKind::OwnedRecordVector(_) | ResourceKind::OwnedEnumVector(_) => {
                let record = OwnedVectorRootSlot::check(leaf.kind.owned_vector_root().unwrap())
                    .ok_or_else(|| bad("owned vector resource domain", span))?
                    .raw();
                queries.validate_owned(heap, record, &source[source_extent], span)?;
            }
            ResourceKind::ByteBuf => {
                heap.descriptor(&source[source_extent])
                    .map_err(|_| bad("resource descriptor", span))?;
            }
            ResourceKind::File => {
                heap.file_descriptor(&source[source_extent])
                    .map_err(|_| bad("file descriptor", span))?;
            }
            ResourceKind::Vector(_) | ResourceKind::RecordVector(_) => {
                let ty = match leaf.kind {
                    ResourceKind::Vector(ty) => super::super::vector_type::VectorTy::Scalar(ty),
                    ResourceKind::RecordVector(id) => {
                        super::super::vector_type::VectorTy::Record(RecordId(id as usize))
                    }
                    _ => unreachable!(),
                };
                let element = super::super::vector_type::checked(ty, declarations)
                    .map_err(vector_type_error_mapper(&span))?;
                let value = heap
                    .vector_descriptor(&source[source_extent], element)
                    .map_err(|_| bad("resource descriptor", span))?;
                heap.vector_validate(&value, element)
                    .map_err(|_| bad("resource descriptor", span))?;
            }
        }
    }
    Ok(())
}

/// Physical descriptor transfer after complete preflight. Ownership-state
/// transitions, rather than stale bytes in moved storage, determine liveness.
pub(super) fn copy<'a, Q: ResourceQueries<'a>>(
    queries: Q,
    aggregate: AggregateTy,
    source: &[u8],
    destination: &mut [u8],
    span: Span,
) -> Result<()> {
    for leaf in queries
        .leaves(aggregate, tag_reader(source))
        .map_err(|_| bad("resource layout", span))?
    {
        let leaf = leaf.map_err(|_| bad("resource active alternative", span))?;
        let source_extent = extent(leaf.offset, source.len(), span)?;
        let destination_extent = extent(leaf.offset, destination.len(), span)?;
        destination[destination_extent].copy_from_slice(&source[source_extent]);
    }
    Ok(())
}

/// No allocation and no fuel debit. Invalid private bytes fail before the
/// first release; verified unique resource obligations cannot become invalid
/// while the execution is exclusively borrowed by this operation.
pub(super) fn release<'a, Q: ResourceQueries<'a>>(
    mut queries: Q,
    heap: &mut byte_buffer::Heap,
    aggregate: AggregateTy,
    bytes: &[u8],
    span: Span,
) -> Result<()> {
    preflight_impl(&mut queries, heap, aggregate, bytes, bytes.len(), span)?;
    let declarations = queries.declarations();
    for leaf in queries
        .leaves(aggregate, tag_reader(bytes))
        .map_err(|_| bad("resource layout", span))?
    {
        let leaf = leaf.map_err(|_| bad("resource active alternative", span))?;
        let range = extent(leaf.offset, bytes.len(), span)?;
        match leaf.kind {
            ResourceKind::OwnedRecordVector(_) | ResourceKind::OwnedEnumVector(_) => {
                let record = OwnedVectorRootSlot::check(leaf.kind.owned_vector_root().unwrap())
                    .ok_or_else(|| bad("owned vector resource domain", span))?
                    .raw();
                queries.release_owned(heap, record, &bytes[range], span)?;
            }
            ResourceKind::ByteBuf => {
                let mut value = heap
                    .descriptor(&bytes[range])
                    .map_err(|_| bad("resource descriptor", span))?;
                heap.release(&mut value)
                    .map_err(|_| bad("resource release", span))?;
            }
            ResourceKind::File => {
                let mut value = heap
                    .file_descriptor(&bytes[range])
                    .map_err(|_| bad("file descriptor", span))?;
                let _ = heap
                    .file_close(&mut value)
                    .map_err(|_| bad("file release", span))?;
            }
            ResourceKind::Vector(_) | ResourceKind::RecordVector(_) => {
                let ty = match leaf.kind {
                    ResourceKind::Vector(ty) => super::super::vector_type::VectorTy::Scalar(ty),
                    ResourceKind::RecordVector(id) => {
                        super::super::vector_type::VectorTy::Record(RecordId(id as usize))
                    }
                    _ => unreachable!(),
                };
                let element = super::super::vector_type::checked(ty, declarations)
                    .map_err(vector_type_error_mapper(&span))?;
                let mut value = heap
                    .vector_descriptor(&bytes[range], element)
                    .map_err(|_| bad("resource descriptor", span))?;
                heap.vector_release(&mut value, element)
                    .map_err(|_| bad("resource release", span))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "execute_resource_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "execute_vector_resource_tests.rs"]
mod vector_tests;
