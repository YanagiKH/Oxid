//! Built-in resource ownership inside the reference consumer. All layouts
//! originate in checked declarations; no scalar can act as an allocation ID.
use super::*;

fn leaves<'a>(
    declarations: &'a Declarations,
    aggregate: AggregateTy,
    bytes: &'a [u8],
) -> std::result::Result<
    ResourceLeaves<'a, impl FnMut(usize) -> std::result::Result<usize, DeclarationError> + 'a>,
    DeclarationError,
> {
    declarations.resource_leaves(aggregate, |offset| {
        let end = offset
            .checked_add(4)
            .ok_or(DeclarationError::LayoutOverflow)?;
        let tag = bytes
            .get(offset..end)
            .ok_or(DeclarationError::LayoutOverflow)?;
        Ok(u32::from_le_bytes(tag.try_into().unwrap()) as usize)
    })
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
        heap.descriptor(&source[source_extent])
            .map_err(|_| bad("resource descriptor", span))?;
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
        let mut value = heap
            .descriptor(&bytes[range])
            .map_err(|_| bad("resource descriptor", span))?;
        heap.release(&mut value)
            .map_err(|_| bad("resource release", span))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "execute_resource_tests.rs"]
mod tests;
