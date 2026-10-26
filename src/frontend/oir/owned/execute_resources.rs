//! Built-in resource ownership inside the reference consumer. All layouts
//! originate in checked declarations; no scalar can act as an allocation ID.
use super::*;

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
    vector_element: byte_buffer::VectorElement,
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
    resource_walk_control_bytes(&reader)
        + mapped_factory_bytes(&reader)
        + size_of::<ReferenceWalkCarriers>()
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

pub(super) fn preflight(
    declarations: &Declarations,
    heap: &byte_buffer::Heap,
    aggregate: AggregateTy,
    source: &[u8],
    destination_length: usize,
    span: Span,
) -> Result<()> {
    for leaf in leaves(declarations, aggregate, source).map_err(|_| bad("resource layout", span))? {
        let leaf = leaf.map_err(|_| bad("resource active alternative", span))?;
        let source_extent = extent(leaf.offset, source.len(), span)?;
        extent(leaf.offset, destination_length, span)?;
        match leaf.kind {
            ResourceKind::ByteBuf => {
                heap.descriptor(&source[source_extent])
                    .map_err(|_| bad("resource descriptor", span))?;
            }
            ResourceKind::File => {
                heap.file_descriptor(&source[source_extent])
                    .map_err(|_| bad("file descriptor", span))?;
            }
            ResourceKind::Vector(ty) => {
                let element = super::super::vector_type::element(ty);
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
pub(super) fn copy(
    declarations: &Declarations,
    aggregate: AggregateTy,
    source: &[u8],
    destination: &mut [u8],
    span: Span,
) -> Result<()> {
    for leaf in leaves(declarations, aggregate, source).map_err(|_| bad("resource layout", span))? {
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
pub(super) fn release(
    declarations: &Declarations,
    heap: &mut byte_buffer::Heap,
    aggregate: AggregateTy,
    bytes: &[u8],
    span: Span,
) -> Result<()> {
    preflight(declarations, heap, aggregate, bytes, bytes.len(), span)?;
    for leaf in leaves(declarations, aggregate, bytes).map_err(|_| bad("resource layout", span))? {
        let leaf = leaf.map_err(|_| bad("resource active alternative", span))?;
        let range = extent(leaf.offset, bytes.len(), span)?;
        match leaf.kind {
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
            ResourceKind::Vector(ty) => {
                let element = super::super::vector_type::element(ty);
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
