//! Heap storage for finite owned-record vectors, separate from Copy vectors.
//!
//! A vector owns its active canonical record cells. Its children remain in the
//! shared intrusive registry. Raw staging nodes never own children; committing
//! growth transfers that ownership without releasing the old descriptors.
use super::{
    alloc, dealloc, ptr, slice, Boundary, Failure, GrowStatus, Header, Heap, Layout, NonNull,
    Result,
};
use crate::frontend::oir::owned::owned_record_codec::{self, Element};

pub(super) const OWNED_VECTOR_NODE: u32 = u32::MAX - 2;
pub(super) const OWNED_VECTOR_HEADER_BYTES: usize = 64;
const ALIGNMENT: usize = 16;

/// A distinct move-only descriptor domain. Neither a byte buffer nor the old
/// scalar/Copy vector descriptor can establish owned-record element authority.
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub(in crate::frontend::oir::owned) struct OwnedVectorDescriptor {
    identity: u64,
    length: i32,
    capacity: i32,
}
impl OwnedVectorDescriptor {
    pub(in crate::frontend::oir::owned) const fn empty() -> Self {
        Self {
            identity: 0,
            length: 0,
            capacity: 0,
        }
    }

    pub(in crate::frontend::oir::owned) fn encoding(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&self.identity.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.length.to_le_bytes());
        bytes[12..].copy_from_slice(&self.capacity.to_le_bytes());
        bytes
    }

    /// Decode inert bytes without registry authority or a heap walk. Every
    /// consuming/query/validation operation independently authenticates this
    /// claim; the sealed consumer retains the real language owner separately.
    pub(in crate::frontend::oir::owned) fn decode(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.get(..16).ok_or(Failure::Descriptor)?;
        Ok(Self {
            identity: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            length: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            capacity: i32::from_le_bytes(bytes[12..].try_into().unwrap()),
        })
    }
}

#[repr(C, align(16))]
struct OwnedVectorHeader {
    prefix: Header,
    capacity: u32,
    record: u32,
    stride: u64,
    allocation_bytes: u64,
    identity_offset: u64,
}
const _: () = {
    assert!(std::mem::size_of::<OwnedVectorDescriptor>() == 16);
    assert!(std::mem::align_of::<OwnedVectorDescriptor>() == 8);
    assert!(std::mem::size_of::<OwnedVectorHeader>() == OWNED_VECTOR_HEADER_BYTES);
    assert!(std::mem::align_of::<OwnedVectorHeader>() == ALIGNMENT);
    assert!(std::mem::offset_of!(OwnedVectorHeader, capacity) == 32);
    assert!(std::mem::offset_of!(OwnedVectorHeader, record) == 36);
    assert!(std::mem::offset_of!(OwnedVectorHeader, stride) == 40);
    assert!(std::mem::offset_of!(OwnedVectorHeader, allocation_bytes) == 48);
    assert!(std::mem::offset_of!(OwnedVectorHeader, identity_offset) == 56);
};

struct AllocationLayout {
    allocation: Layout,
    identity_offset: usize,
    identity_slots: usize,
}

/// Complete fixed transports for the new heap transaction lane. Identity
/// payload is retained in each already charged node tail, never allocated as
/// operation scratch. The codec separately inventories its bounded walkers.
struct ControlCarriers<'a> {
    heap_arguments: [&'a Heap; 2],
    mutable_heap_argument: &'a mut Heap,
    value_arguments: [&'a OwnedVectorDescriptor; 2],
    mutable_value_argument: &'a mut OwnedVectorDescriptor,
    output_argument: &'a mut OwnedVectorDescriptor,
    elements: [Element<'a>; 3],
    descriptor: OwnedVectorDescriptor,
    output: OwnedVectorDescriptor,
    decoding_return: Result<OwnedVectorDescriptor>,
    caller_decoding_return: Result<OwnedVectorDescriptor>,
    controls: [(usize, usize); 3],
    borrowed_controls: &'a [(usize, usize)],
    selected_control: (usize, usize),
    extent_return: Result<(usize, usize)>,
    allocation_layout: AllocationLayout,
    allocation_return: Result<AllocationLayout>,
    node: Option<NonNull<Header>>,
    located_return: Result<Option<NonNull<Header>>>,
    selected_node: NonNull<OwnedVectorHeader>,
    source: &'a mut [u8],
    destination: &'a mut [u8],
    identities: &'a mut [u64],
    fuel: &'a mut usize,
    count_result: &'a mut i32,
    work_returns: [Result<usize>; 3],
    operation_returns: [Result<()>; 3],
    growth_return: Result<GrowStatus>,
    caller_growth_return: Result<GrowStatus>,
    returned_encoding: [u8; 16],
    counters: [usize; 12],
}
pub(in crate::frontend::oir::owned) const fn control_bytes() -> usize {
    std::mem::size_of::<ControlCarriers<'_>>() + owned_record_codec::control_bytes()
}
impl AllocationLayout {
    fn size(&self) -> usize {
        self.allocation.size()
    }
}
fn allocation_layout(element: Element<'_>, capacity: i32) -> Option<AllocationLayout> {
    let capacity = usize::try_from(capacity).ok()?;
    let payload = capacity.checked_mul(element.size())?;
    let identity_offset = OWNED_VECTOR_HEADER_BYTES
        .checked_add(payload)?
        .checked_add(std::mem::align_of::<u64>() - 1)?
        & !(std::mem::align_of::<u64>() - 1);
    let identity_slots = capacity.checked_mul(element.identity_capacity())?;
    let bytes = identity_offset
        .checked_add(identity_slots.checked_mul(std::mem::size_of::<u64>())?)?
        .checked_add(ALIGNMENT - 1)?
        & !(ALIGNMENT - 1);
    Some(AllocationLayout {
        allocation: Layout::from_size_align(bytes, ALIGNMENT).ok()?,
        identity_offset,
        identity_slots,
    })
}

fn data(node: NonNull<Header>) -> *mut u8 {
    // SAFETY: Registration and locate_owned_vector alone supply these nodes.
    unsafe { node.cast::<OwnedVectorHeader>().as_ptr().add(1).cast() }
}

fn overlaps(a: usize, a_len: usize, b: usize, b_len: usize) -> bool {
    a_len != 0 && b_len != 0 && if a <= b { b - a < a_len } else { a - b < b_len }
}

/// Only a live node reached through the intrusive registry supplies this
/// address. Descriptor bytes are never interpreted as a pointer or extent.
unsafe fn registered_extent(node: NonNull<Header>) -> Result<(usize, usize)> {
    let header = unsafe { node.as_ref() };
    let charge = match header.capacity {
        super::files::FILE_NODE => super::ALLOCATION_CHARGE,
        super::vectors::VECTOR_NODE => unsafe { super::vectors::node_charge(header) },
        OWNED_VECTOR_NODE => unsafe { node_charge(header) },
        capacity => u64::from(capacity) + super::ALLOCATION_CHARGE,
    };
    let bytes = usize::try_from(charge).map_err(|_| Failure::Descriptor)?;
    let start = node.as_ptr() as usize;
    if start.checked_add(bytes).is_none() {
        return Err(Failure::Descriptor);
    }
    Ok((start, bytes))
}

/// Registry-only raw cleanup. Children have independent registrations, so
/// fatal cleanup and failed staging require no drop walk, recursion or scratch.
pub(super) unsafe fn node_charge(header: &Header) -> u64 {
    debug_assert_eq!(header.capacity, OWNED_VECTOR_NODE);
    unsafe { (*(header as *const Header).cast::<OwnedVectorHeader>()).allocation_bytes }
}
pub(super) unsafe fn deallocate_node(node: NonNull<Header>) {
    let bytes = unsafe { node_charge(node.as_ref()) };
    let layout = Layout::from_size_align(
        usize::try_from(bytes).expect("registered owned vector size"),
        ALIGNMENT,
    )
    .expect("registered owned vector layout");
    unsafe { dealloc(node.as_ptr().cast(), layout) };
}

impl Heap {
    fn locate_owned_vector(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
    ) -> Result<Option<NonNull<Header>>> {
        if value.identity == 0 {
            return if value.length == 0 && value.capacity == 0 {
                Ok(None)
            } else {
                Err(Failure::Descriptor)
            };
        }
        if value.length < 0 || value.capacity <= 0 || value.length > value.capacity {
            return Err(Failure::Descriptor);
        }
        let mut cursor = self.head;
        while let Some(node) = NonNull::new(cursor) {
            let header = unsafe { node.as_ref() };
            if header.identity == value.identity {
                if header.capacity != OWNED_VECTOR_NODE
                    || !(node.as_ptr() as usize).is_multiple_of(ALIGNMENT)
                {
                    return Err(Failure::Descriptor);
                }
                let vector = unsafe { node.cast::<OwnedVectorHeader>().as_ref() };
                let layout =
                    allocation_layout(element, value.capacity).ok_or(Failure::Descriptor)?;
                return if vector.record == element.storage_key()
                    && vector.stride == element.size() as u64
                    && vector.capacity == value.capacity as u32
                    && header.initialized == value.length as u32
                    && vector.allocation_bytes == layout.size() as u64
                    && vector.identity_offset == layout.identity_offset as u64
                {
                    Ok(Some(node))
                } else {
                    Err(Failure::Descriptor)
                };
            }
            cursor = header.next;
        }
        Err(Failure::Descriptor)
    }

    pub(in crate::frontend::oir::owned) fn owned_vector_descriptor(
        &self,
        bytes: &[u8],
        element: Element<'_>,
    ) -> Result<OwnedVectorDescriptor> {
        let value = OwnedVectorDescriptor::decode(bytes)?;
        self.locate_owned_vector(&value, element)?;
        Ok(value)
    }

    /// One paid registry validation for ordinary transfers of this resource.
    /// No source/output owner bytes or fuel change on any refusal. This is the
    /// same R2 admission/lookup request as the native core:4+3*live_nodes.
    pub(in crate::frontend::oir::owned) fn owned_vector_validate(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
    ) -> Result<()> {
        let remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        self.locate_owned_vector(value, element)?;
        *fuel = remaining;
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::frontend::oir::owned) fn owned_vector_len(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
    ) -> Result<i32> {
        self.locate_owned_vector(value, element)?;
        Ok(value.length)
    }

    #[cfg(test)]
    pub(in crate::frontend::oir::owned) fn owned_vector_capacity(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
    ) -> Result<i32> {
        self.locate_owned_vector(value, element)?;
        Ok(value.capacity)
    }

    /// Production queries admit their actual caller-owned output range and
    /// its four-byte write, as well as descriptor, fuel and registry work.
    fn owned_vector_query_into(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
        result: &mut i32,
        capacity: bool,
    ) -> Result<()> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
                (result as *const _ as usize, std::mem::size_of_val(result)),
            ],
            *fuel,
        )?;
        let remaining = control_remaining
            .checked_sub(std::mem::size_of_val(result))
            .ok_or(Failure::Fuel)?;
        self.locate_owned_vector(value, element)?;
        *result = if capacity {
            value.capacity
        } else {
            value.length
        };
        *fuel = remaining;
        Ok(())
    }

    pub(in crate::frontend::oir::owned) fn owned_vector_len_into(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
        result: &mut i32,
    ) -> Result<()> {
        self.owned_vector_query_into(value, element, fuel, result, false)
    }

    pub(in crate::frontend::oir::owned) fn owned_vector_capacity_into(
        &self,
        value: &OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
        result: &mut i32,
    ) -> Result<()> {
        self.owned_vector_query_into(value, element, fuel, result, true)
    }

    /// Reject caller-owned control/scratch against every registered block.
    /// The selected vector's internal identity tail is not an external range;
    /// only preflight constructs its authenticated, disjoint internal view.
    /// Language loan admission is additionally required in the sealed consumer.
    fn owned_vector_disjoint(&self, ranges: &[(usize, usize)]) -> Result<()> {
        for (index, &(start, bytes)) in ranges.iter().enumerate() {
            if start.checked_add(bytes).is_none()
                || overlaps(
                    start,
                    bytes,
                    self as *const Self as usize,
                    std::mem::size_of::<Self>(),
                )
                || ranges[..index]
                    .iter()
                    .any(|&(other, count)| overlaps(start, bytes, other, count))
            {
                return Err(Failure::Descriptor);
            }
            let mut cursor = self.head;
            while let Some(node) = NonNull::new(cursor) {
                // SAFETY: Only this heap links live, independently allocated
                // registry nodes; no descriptor-derived pointer is followed.
                let (other, count) = unsafe { registered_extent(node)? };
                if overlaps(start, bytes, other, count) {
                    return Err(Failure::Descriptor);
                }
                cursor = unsafe { node.as_ref().next };
            }
        }
        Ok(())
    }

    /// Logical work for the complete external-range admission and the one
    /// descriptor lookup that follows it: one fixed admission, one Heap check
    /// per range, all pair comparisons, all range/node extent comparisons and
    /// at most one full registry lookup. It is admitted before either walk.
    fn owned_vector_control_work(&self, ranges: usize) -> Result<usize> {
        let nodes = usize::try_from(self.ledger.live_allocations).map_err(|_| Failure::Fuel)?;
        let pairs = ranges
            .checked_mul(ranges.saturating_sub(1))
            .map(|pairs| pairs / 2)
            .ok_or(Failure::Fuel)?;
        nodes
            .checked_mul(ranges.checked_add(1).ok_or(Failure::Fuel)?)
            .and_then(|work| work.checked_add(ranges))
            .and_then(|work| work.checked_add(pairs))
            .and_then(|work| work.checked_add(1))
            .ok_or(Failure::Fuel)
    }

    fn owned_vector_admit_controls(&self, ranges: &[(usize, usize)], fuel: usize) -> Result<usize> {
        let remaining = fuel
            .checked_sub(self.owned_vector_control_work(ranges.len())?)
            .ok_or(Failure::Fuel)?;
        self.owned_vector_disjoint(ranges)?;
        Ok(remaining)
    }

    /// Complete active-shape and resource-kind preflight. A single scratch
    /// non-owning tail covers the entire vector and optional incoming record, so two
    /// active cells cannot acquire the same child identity.
    fn preflight_owned_vector(
        &mut self,
        node: Option<NonNull<Header>>,
        length: i32,
        element: Element<'_>,
        incoming: Option<&[u8]>,
    ) -> Result<()> {
        let identities = if let Some(node) = node {
            let header = unsafe { node.cast::<OwnedVectorHeader>().as_ref() };
            let layout =
                allocation_layout(element, header.capacity as i32).ok_or(Failure::Descriptor)?;
            // locate_owned_vector authenticated this exact header and layout.
            // The aligned tail is disjoint from all record cells and owns no
            // child. No operation in preflight can unlink its containing node.
            unsafe {
                slice::from_raw_parts_mut(
                    node.as_ptr()
                        .cast::<u8>()
                        .add(layout.identity_offset)
                        .cast::<u64>(),
                    layout.identity_slots,
                )
            }
        } else {
            &mut []
        };
        let mut count = 0usize;
        if let Some(node) = node {
            let bytes = length as usize * element.size();
            let source = unsafe { slice::from_raw_parts(data(node), bytes) };
            for record in source.chunks_exact(element.size()) {
                count += element.preflight(record, self, &mut identities[count..])?;
            }
        }
        if let Some(incoming) = incoming {
            count += element.preflight(incoming, self, &mut identities[count..])?;
        }
        owned_record_codec::unique_identities(&mut identities[..count])
    }

    /// Reserve has no incoming owner. Failure therefore leaves both the old
    /// vector and every caller-owned record unchanged. Scratch is prepaid in
    /// each vector allocation's aligned non-owning tail.
    pub(in crate::frontend::oir::owned) fn owned_vector_try_reserve(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        additional: i32,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        let old = self.locate_owned_vector(value, element)?;
        if additional < 0 {
            return Err(Failure::Bounds);
        }
        let Some(required) = value.length.checked_add(additional) else {
            *fuel = control_remaining;
            return Ok(GrowStatus::CapacityOverflow);
        };
        let capacity = super::growth_capacity(value.capacity, required)?;
        if capacity == value.capacity {
            *fuel = control_remaining;
            return Ok(GrowStatus::Ok);
        }
        let Some(layout) = allocation_layout(element, capacity) else {
            *fuel = control_remaining;
            return Ok(GrowStatus::CapacityOverflow);
        };
        let copy_bytes = (value.length as usize)
            .checked_mul(element.size())
            .ok_or(Failure::Fuel)?;
        let writes = layout.size().checked_add(copy_bytes).ok_or(Failure::Fuel)?;
        let work = self.owned_vector_work(element, value.length as usize, writes)?;
        let remaining = control_remaining.checked_sub(work).ok_or(Failure::Fuel)?;
        self.preflight_owned_vector(old, value.length, element, None)?;
        *fuel = remaining;
        if self
            .ledger
            .live_bytes
            .checked_add(layout.size() as u64)
            .is_none_or(|bytes| bytes > self.limit)
        {
            return Ok(GrowStatus::MemoryLimit);
        }
        if self.next_identity == u64::MAX {
            return Err(Failure::IdentityExhausted);
        }
        if self.fail(Boundary::Allocate) {
            return Ok(GrowStatus::AllocationFailed);
        }
        let Some(node) =
            NonNull::new(unsafe { alloc(layout.allocation).cast::<OwnedVectorHeader>() })
        else {
            return Ok(GrowStatus::AllocationFailed);
        };
        unsafe {
            ptr::write(
                node.as_ptr(),
                OwnedVectorHeader {
                    prefix: Header {
                        next: self.head,
                        previous: ptr::null_mut(),
                        identity: self.next_identity,
                        capacity: OWNED_VECTOR_NODE,
                        initialized: 0,
                    },
                    capacity: capacity as u32,
                    record: element.storage_key(),
                    stride: element.size() as u64,
                    allocation_bytes: layout.size() as u64,
                    identity_offset: layout.identity_offset as u64,
                },
            );
            if !self.head.is_null() {
                (*self.head).previous = node.as_ptr().cast();
            }
        }
        self.head = node.as_ptr().cast();
        self.next_identity += 1;
        self.ledger.live_bytes += layout.size() as u64;
        self.ledger.live_allocations += 1;
        self.ledger.allocations += 1;
        self.ledger.peak_bytes = self.ledger.peak_bytes.max(self.ledger.live_bytes);
        if self.fail(Boundary::Initialize) {
            self.unlink(node.cast());
            return Ok(GrowStatus::AllocationFailed);
        }
        // Every inactive slot and alignment byte is canonical zero. No copied
        // child descriptor becomes live before the final ownership commit.
        unsafe {
            ptr::write_bytes(
                data(node.cast()),
                0,
                layout.size() - OWNED_VECTOR_HEADER_BYTES,
            );
        }
        if self.fail(Boundary::Commit) {
            self.unlink(node.cast());
            return Ok(GrowStatus::AllocationFailed);
        }
        // There are no fallible operations after this point. The old raw block
        // is unlinked without walking its children, which now belong to node.
        unsafe {
            if let Some(old) = old {
                ptr::copy_nonoverlapping(
                    data(old),
                    data(node.cast()),
                    value.length as usize * element.size(),
                );
            }
            (*node.as_ptr()).prefix.initialized = value.length as u32;
            value.identity = (*node.as_ptr()).prefix.identity;
        }
        value.capacity = capacity;
        if let Some(old) = old {
            self.unlink(old);
        }
        Ok(GrowStatus::Ok)
    }

    /// Allocation-free move into a reserved slot. Insufficient capacity is a
    /// checked runtime bounds failure; it never consumes the incoming owner.
    /// The consumer must preadmit the mutable vector loan and source owner.
    pub(in crate::frontend::oir::owned) fn owned_vector_push_reserved(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        source: &mut [u8],
        fuel: &mut usize,
    ) -> Result<()> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (source.as_ptr() as usize, source.len()),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        let old = self.locate_owned_vector(value, element)?;
        if value.length == value.capacity {
            return Err(Failure::Bounds);
        }
        if source.len() != element.size() {
            return Err(Failure::Descriptor);
        }
        let writes = element.size().checked_mul(3).ok_or(Failure::Fuel)?;
        let work = self.owned_vector_work(element, value.length as usize + 1, writes)?;
        let remaining = control_remaining.checked_sub(work).ok_or(Failure::Fuel)?;
        self.preflight_owned_vector(old, value.length, element, Some(source))?;
        let mut node = old.ok_or(Failure::Descriptor)?;
        let target = unsafe {
            slice::from_raw_parts_mut(
                data(node).add(value.length as usize * element.size()),
                element.size(),
            )
        };
        // Only inactive storage is written here. The shape has already been
        // proven; no fallible step remains after ownership is published.
        element.canonical_copy(source, target)?;
        *fuel = remaining;
        source.fill(0);
        value.length += 1;
        unsafe { node.as_mut().initialized = value.length as u32 };
        Ok(())
    }

    /// Move the last initialized record into admitted unowned result scratch.
    /// Empty vectors fail before modifying either result or vector storage.
    pub(in crate::frontend::oir::owned) fn owned_vector_pop_last(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        destination: &mut [u8],
        fuel: &mut usize,
    ) -> Result<()> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (destination.as_ptr() as usize, destination.len()),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        let old = self.locate_owned_vector(value, element)?;
        if value.length == 0 {
            return Err(Failure::Bounds);
        }
        if destination.len() != element.size() {
            return Err(Failure::Descriptor);
        }
        let writes = element.size().checked_mul(2).ok_or(Failure::Fuel)?;
        let work = self.owned_vector_work(element, value.length as usize, writes)?;
        let remaining = control_remaining.checked_sub(work).ok_or(Failure::Fuel)?;
        self.preflight_owned_vector(old, value.length, element, None)?;
        let mut node = old.ok_or(Failure::Descriptor)?;
        // Complete result extent and all source cells were preflighted. Stored
        // records are canonical, so this infallible move also preserves zeros.
        unsafe {
            let source = data(node).add((value.length as usize - 1) * element.size());
            ptr::copy_nonoverlapping(source, destination.as_mut_ptr(), element.size());
            ptr::write_bytes(source, 0, element.size());
            node.as_mut().initialized -= 1;
        }
        *fuel = remaining;
        value.length -= 1;
        Ok(())
    }

    pub(in crate::frontend::oir::owned) fn owned_vector_truncate(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        length: i32,
        fuel: &mut usize,
    ) -> Result<()> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        let old = self.locate_owned_vector(value, element)?;
        if length < 0 {
            return Err(Failure::Bounds);
        }
        if length >= value.length {
            *fuel = control_remaining;
            return Ok(());
        }
        let removed = (value.length - length) as usize;
        let writes = removed.checked_mul(element.size()).ok_or(Failure::Fuel)?;
        // Preflight checks all old cells once. Releasing each removed child
        // performs at most two further identity lookups, with a shrinking list.
        let release_search = removed
            .checked_mul(element.identity_capacity())
            .and_then(|children| children.checked_mul(2))
            .and_then(|lookups| {
                lookups.checked_mul(usize::try_from(self.ledger.live_allocations).ok()?)
            })
            .ok_or(Failure::Fuel)?;
        let work = self
            .owned_vector_work(element, value.length as usize, writes)?
            .checked_add(release_search)
            .ok_or(Failure::Fuel)?;
        let remaining = control_remaining.checked_sub(work).ok_or(Failure::Fuel)?;
        self.preflight_owned_vector(old, value.length, element, None)?;
        *fuel = remaining;
        self.commit_owned_vector_truncate(old, value, element, length);
        Ok(())
    }

    fn commit_owned_vector_truncate(
        &mut self,
        old: Option<NonNull<Header>>,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        length: i32,
    ) {
        if let Some(mut node) = old {
            // The vector allocation cannot be a child: this codec admits only
            // the existing leaf resource kinds. Child release keeps it alive.
            for index in length..value.length {
                let source = unsafe {
                    slice::from_raw_parts(
                        data(node).add(index as usize * element.size()),
                        element.size(),
                    )
                };
                element.release_children(source, self);
            }
            unsafe {
                ptr::write_bytes(
                    data(node).add(length as usize * element.size()),
                    0,
                    (value.length - length) as usize * element.size(),
                );
                node.as_mut().initialized = length as u32;
            }
        }
        value.length = length;
    }

    /// Owner-exit maintenance has no language fuel dependency: after complete
    /// validation it releases only finite active registered children, without
    /// invoking user code or allocating scratch. Fatal cleanup is a separate
    /// flat raw-node walk and never performs this child traversal.
    pub(in crate::frontend::oir::owned) fn owned_vector_release(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
    ) -> Result<()> {
        let old = self.locate_owned_vector(value, element)?;
        self.owned_vector_disjoint(&[(value as *const _ as usize, std::mem::size_of_val(value))])?;
        self.preflight_owned_vector(old, value.length, element, None)?;
        self.commit_owned_vector_truncate(old, value, element, 0);
        if let Some(node) = old {
            self.unlink(node);
        }
        *value = OwnedVectorDescriptor::empty();
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::frontend::oir::owned) fn owned_vector_take(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
    ) -> Result<OwnedVectorDescriptor> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
            ],
            *fuel,
        )?;
        let remaining = control_remaining
            .checked_sub(2 * std::mem::size_of::<OwnedVectorDescriptor>())
            .ok_or(Failure::Fuel)?;
        self.locate_owned_vector(value, element)?;
        *fuel = remaining;
        Ok(std::mem::replace(value, OwnedVectorDescriptor::empty()))
    }

    /// The checked caller owns unowned result storage. Its full output extent
    /// participates in the same three-range admission as the native adapter.
    pub(in crate::frontend::oir::owned) fn owned_vector_take_into(
        &mut self,
        value: &mut OwnedVectorDescriptor,
        element: Element<'_>,
        fuel: &mut usize,
        result: &mut OwnedVectorDescriptor,
    ) -> Result<()> {
        let control_remaining = self.owned_vector_admit_controls(
            &[
                (value as *const _ as usize, std::mem::size_of_val(value)),
                (fuel as *const _ as usize, std::mem::size_of_val(fuel)),
                (result as *const _ as usize, std::mem::size_of_val(result)),
            ],
            *fuel,
        )?;
        let remaining = control_remaining
            .checked_sub(2 * std::mem::size_of::<OwnedVectorDescriptor>())
            .ok_or(Failure::Fuel)?;
        self.locate_owned_vector(value, element)?;
        *result = std::mem::replace(value, OwnedVectorDescriptor::empty());
        *fuel = remaining;
        Ok(())
    }

    fn owned_vector_work(
        &self,
        element: Element<'_>,
        records: usize,
        copied_bytes: usize,
    ) -> Result<usize> {
        let children = element
            .identity_capacity()
            .checked_mul(records)
            .ok_or(Failure::Fuel)?;
        let search = children
            .checked_mul(usize::try_from(self.ledger.live_allocations).map_err(|_| Failure::Fuel)?)
            .ok_or(Failure::Fuel)?;
        let sort = children
            .checked_mul(usize::BITS as usize - children.leading_zeros() as usize)
            .and_then(|work| work.checked_mul(4))
            .ok_or(Failure::Fuel)?;
        element
            .traversal_work(records)
            .and_then(|work| work.checked_mul(3))
            .and_then(|work| work.checked_add(search))
            .and_then(|work| work.checked_add(sort))
            .and_then(|work| work.checked_add(copied_bytes))
            .ok_or(Failure::Fuel)
    }
}

#[cfg(test)]
#[path = "owned_vector_resource_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "owned_enum_vector_resource_tests.rs"]
mod enum_tests;
