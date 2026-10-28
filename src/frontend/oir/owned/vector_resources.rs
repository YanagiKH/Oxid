//! Typed scalar vectors below the independently verified consumer.
//! This file is a child module of byte_buffer.rs, sharing its heap registry,
//! identity sequence, failure controls, and normalized live-byte ledger.
use super::{
    alloc, dealloc, ptr, slice, Boundary, Failure, GrowStatus, Header, Heap, Layout, NonNull,
    Result,
};

pub(super) const VECTOR_NODE: u32 = u32::MAX - 1;
pub(super) const VECTOR_HEADER_BYTES: usize = 48;
const ALIGNMENT: usize = 16;

/// Closed compiler-to-runtime identities. They must be derived from checked
/// scalar types, never a user's integer, nominal spelling, or byte descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(in crate::frontend::oir::owned) enum Element {
    Bool = 1,
    Unit = 2,
    I8 = 3,
    U8 = 4,
    I16 = 5,
    U16 = 6,
    I32 = 7,
    U32 = 8,
    I64 = 9,
    U64 = 10,
    Isize = 11,
    Usize = 12,
    F32 = 13,
    F64 = 14,
    Char = 15,
    I128 = 16,
    U128 = 17,
}
impl Element {
    pub(in crate::frontend::oir::owned) const fn from_tag(tag: u32) -> Option<Self> {
        Some(match tag {
            1 => Self::Bool,
            2 => Self::Unit,
            3 => Self::I8,
            4 => Self::U8,
            5 => Self::I16,
            6 => Self::U16,
            7 => Self::I32,
            8 => Self::U32,
            9 => Self::I64,
            10 => Self::U64,
            11 => Self::Isize,
            12 => Self::Usize,
            13 => Self::F32,
            14 => Self::F64,
            15 => Self::Char,
            16 => Self::I128,
            17 => Self::U128,
            _ => return None,
        })
    }
    pub(in crate::frontend::oir::owned) const fn tag(self) -> u32 {
        self as u32
    }
    pub(in crate::frontend::oir::owned) const fn size(self) -> usize {
        match self {
            Self::Bool | Self::Unit | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 | Self::Char => 4,
            Self::I64 | Self::U64 | Self::Isize | Self::Usize | Self::F64 => 8,
            Self::I128 | Self::U128 => 16,
        }
    }
    pub(in crate::frontend::oir::owned) const fn align(self) -> usize {
        self.size()
    }
    fn canonical(self, bytes: &[u8]) -> bool {
        if !bytes.len().is_multiple_of(self.size()) {
            return false;
        }
        match self {
            Self::Bool => bytes.iter().all(|byte| *byte <= 1),
            Self::Unit => bytes.iter().all(|byte| *byte == 0),
            Self::Char => bytes
                .as_chunks::<4>()
                .0
                .iter()
                .all(|word| char::from_u32(u32::from_le_bytes(*word)).is_some()),
            _ => true, // Integers and floats preserve every admitted bit pattern.
        }
    }
}

mod sealed {
    pub trait Sealed {}
}
pub(in crate::frontend::oir::owned) trait VectorType:
    sealed::Sealed + Copy
{
    fn tag(self) -> u32;
    fn size(self) -> usize;
    fn canonical(self, bytes: &[u8]) -> bool;
    fn validation_work(self, bytes: usize) -> Option<usize> {
        Some(bytes)
    }
}
impl sealed::Sealed for Element {}
impl VectorType for Element {
    fn tag(self) -> u32 {
        self.tag()
    }
    fn size(self) -> usize {
        self.size()
    }
    fn canonical(self, bytes: &[u8]) -> bool {
        self.canonical(bytes)
    }
}

/// A runtime element witness can only borrow an independently checked table.
#[derive(Clone, Copy)]
pub(in crate::frontend::oir::owned) struct RecordElement<'a> {
    declarations: &'a crate::frontend::oir::owned_types::Declarations,
    record: crate::frontend::oir::owned_types::RecordId,
    bytes: usize,
}
impl<'a> RecordElement<'a> {
    pub(in crate::frontend::oir::owned) fn check(
        declarations: &'a crate::frontend::oir::owned_types::Declarations,
        record: crate::frontend::oir::owned_types::RecordId,
    ) -> Result<Self> {
        if record.0 >= 4096 {
            return Err(Failure::Descriptor);
        }
        let bytes = declarations
            .record_packed_bytes(record)
            .map_err(|_| Failure::Descriptor)?;
        Ok(Self {
            declarations,
            record,
            bytes,
        })
    }
    pub(in crate::frontend::oir::owned) fn tag(self) -> u32 {
        0x8000_0000 | self.record.0 as u32
    }
    pub(in crate::frontend::oir::owned) fn size(self) -> usize {
        self.bytes
    }
    pub(in crate::frontend::oir::owned) fn canonical(self, bytes: &[u8]) -> bool {
        if !bytes.len().is_multiple_of(self.bytes) {
            return false;
        }
        for value in bytes.chunks_exact(self.bytes) {
            let Ok(leaves) =
                self.declarations
                    .leaves(crate::frontend::oir::owned_types::AggregateTy::Record(
                        self.record,
                    ))
            else {
                return false;
            };
            let mut cursor = 0usize;
            for leaf in leaves {
                let scalar = super::super::vector_type::element(leaf.ty);
                let end = cursor + scalar.size();
                if end > value.len() || !scalar.canonical(&value[cursor..end]) {
                    return false;
                }
                cursor = end;
            }
            if cursor != value.len() {
                return false;
            }
        }
        true
    }
}
impl sealed::Sealed for RecordElement<'_> {}
impl VectorType for RecordElement<'_> {
    fn validation_work(self, bytes: usize) -> Option<usize> {
        // Every initialized leaf occupies >=1 dense byte. A finite tree of
        // depth D with L leaves has at most (D+1)*L visited nodes, even for
        // unary record chains. Two iterator transitions plus scalar admission
        // are bounded by (2*D+3)*bytes; this does not assume L <= D.
        bytes.checked_mul(2 * crate::frontend::oir::owned_types::MAX_CONTAINMENT_DEPTH + 3)
    }
    fn tag(self) -> u32 {
        self.tag()
    }
    fn size(self) -> usize {
        self.size()
    }
    fn canonical(self, bytes: &[u8]) -> bool {
        self.canonical(bytes)
    }
}

/// A closed sum of scalar authority and a checked nominal record witness.
#[derive(Clone, Copy)]
pub(in crate::frontend::oir::owned) enum CheckedElement<'a> {
    Scalar(Element),
    Record(RecordElement<'a>),
}
impl CheckedElement<'_> {
    pub(in crate::frontend::oir::owned) fn tag(self) -> u32 {
        VectorType::tag(self)
    }
    pub(in crate::frontend::oir::owned) fn size(self) -> usize {
        VectorType::size(self)
    }
}
impl sealed::Sealed for CheckedElement<'_> {}
impl VectorType for CheckedElement<'_> {
    fn validation_work(self, bytes: usize) -> Option<usize> {
        match self {
            Self::Scalar(value) => value.validation_work(bytes),
            Self::Record(value) => value.validation_work(bytes),
        }
    }
    fn tag(self) -> u32 {
        match self {
            Self::Scalar(value) => value.tag(),
            Self::Record(value) => value.tag(),
        }
    }
    fn size(self) -> usize {
        match self {
            Self::Scalar(value) => value.size(),
            Self::Record(value) => value.size(),
        }
    }
    fn canonical(self, bytes: &[u8]) -> bool {
        match self {
            Self::Scalar(value) => value.canonical(bytes),
            Self::Record(value) => value.canonical(bytes),
        }
    }
}

/// Move-only descriptor, encoded in activations exactly as three integer words.
/// The expected element is supplied from the verified type at every lookup.
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub(in crate::frontend::oir::owned) struct VectorDescriptor {
    identity: u64,
    length: i32,
    capacity: i32,
}
impl VectorDescriptor {
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
    pub(in crate::frontend::oir::owned) fn write(&self, bytes: &mut [u8]) -> Result<()> {
        bytes
            .get_mut(..16)
            .ok_or(Failure::Descriptor)?
            .copy_from_slice(&self.encoding());
        Ok(())
    }
}

#[repr(C, align(16))]
struct VectorHeader {
    prefix: Header,
    capacity: u32,
    element: u32,
    allocation_bytes: u64,
}
const _: () = {
    assert!(std::mem::size_of::<VectorDescriptor>() == 16);
    assert!(std::mem::align_of::<VectorDescriptor>() == 8);
    assert!(std::mem::size_of::<VectorHeader>() == VECTOR_HEADER_BYTES);
    assert!(std::mem::align_of::<VectorHeader>() == ALIGNMENT);
    assert!(std::mem::offset_of!(VectorHeader, capacity) == 32);
};

fn vector_layout(element: impl VectorType, capacity: i32) -> Option<Layout> {
    let payload = usize::try_from(capacity)
        .ok()?
        .checked_mul(element.size())?;
    let bytes = VECTOR_HEADER_BYTES
        .checked_add(payload)?
        .checked_add(ALIGNMENT - 1)?
        & !(ALIGNMENT - 1);
    Layout::from_size_align(bytes, ALIGNMENT).ok()
}
fn vector_data(node: NonNull<Header>) -> *mut u8 {
    // SAFETY: Only locate_vector or registration supplies vector-class nodes.
    unsafe { node.cast::<VectorHeader>().as_ptr().add(1).cast() }
}
fn overlaps(a: usize, a_len: usize, b: usize, b_len: usize) -> bool {
    a_len != 0 && b_len != 0 && if a <= b { b - a < a_len } else { a - b < b_len }
}

/// Registry-only cleanup hooks for Heap::unlink. The allocation, marker and
/// immutable allocation metadata were established by vector registration.
pub(super) unsafe fn node_charge(header: &Header) -> u64 {
    debug_assert_eq!(header.capacity, VECTOR_NODE);
    unsafe { (*(header as *const Header).cast::<VectorHeader>()).allocation_bytes }
}
pub(super) unsafe fn deallocate_node(header: NonNull<Header>) {
    let charge = unsafe { node_charge(header.as_ref()) };
    let layout = Layout::from_size_align(
        usize::try_from(charge).expect("registered vector size"),
        ALIGNMENT,
    )
    .expect("registered vector layout");
    unsafe { dealloc(header.as_ptr().cast(), layout) };
}

enum Mutation<'a> {
    Reserve,
    Extend(&'a [u8]),
    Resize(&'a [u8]),
}
impl Heap {
    fn locate_vector(
        &self,
        value: &VectorDescriptor,
        element: impl VectorType,
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
                if header.capacity != VECTOR_NODE
                    || !(node.as_ptr() as usize).is_multiple_of(ALIGNMENT)
                {
                    return Err(Failure::Descriptor);
                }
                let vector = unsafe { node.cast::<VectorHeader>().as_ref() };
                let layout = vector_layout(element, value.capacity).ok_or(Failure::Descriptor)?;
                return if vector.element == element.tag()
                    && vector.capacity == value.capacity as u32
                    && header.initialized == value.length as u32
                    && vector.allocation_bytes == layout.size() as u64
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
    pub(in crate::frontend::oir::owned) fn vector_descriptor(
        &self,
        bytes: &[u8],
        element: impl VectorType,
    ) -> Result<VectorDescriptor> {
        let bytes = bytes.get(..16).ok_or(Failure::Descriptor)?;
        let value = VectorDescriptor {
            identity: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            length: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            capacity: i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        };
        self.locate_vector(&value, element)?;
        Ok(value)
    }
    pub(in crate::frontend::oir::owned) fn vector_validate(
        &self,
        value: &VectorDescriptor,
        element: impl VectorType,
    ) -> Result<()> {
        self.locate_vector(value, element).map(|_| ())
    }
    pub(in crate::frontend::oir::owned) fn vector_len(
        &self,
        value: &VectorDescriptor,
        element: impl VectorType,
    ) -> Result<i32> {
        self.vector_validate(value, element)?;
        Ok(value.length)
    }
    pub(in crate::frontend::oir::owned) fn vector_capacity(
        &self,
        value: &VectorDescriptor,
        element: impl VectorType,
    ) -> Result<i32> {
        self.vector_validate(value, element)?;
        Ok(value.capacity)
    }
    pub(in crate::frontend::oir::owned) fn vector_range<'a>(
        &'a self,
        value: &VectorDescriptor,
        element: impl VectorType,
        start: i32,
        end: i32,
    ) -> Result<&'a [u8]> {
        let node = self.locate_vector(value, element)?;
        super::bounds(start, end, value.length)?;
        let Some(node) = node else {
            return Ok(&[]);
        };
        let offset = start as usize * element.size();
        let bytes = (end - start) as usize * element.size();
        Ok(unsafe { slice::from_raw_parts(vector_data(node).add(offset), bytes) })
    }
    /// # Safety
    /// The verified caller must maintain scalar representation validity and
    /// exclusive language-loan authority. Store through initialized scalar
    /// scratch; arbitrary bytes are not valid bool/unit/char values.
    pub(in crate::frontend::oir::owned) unsafe fn vector_range_mut<'a>(
        &'a mut self,
        value: &VectorDescriptor,
        element: impl VectorType,
        start: i32,
        end: i32,
    ) -> Result<&'a mut [u8]> {
        let node = self.locate_vector(value, element)?;
        super::bounds(start, end, value.length)?;
        let Some(node) = node else {
            return Ok(&mut []);
        };
        let offset = start as usize * element.size();
        let bytes = (end - start) as usize * element.size();
        Ok(unsafe { slice::from_raw_parts_mut(vector_data(node).add(offset), bytes) })
    }
    pub(in crate::frontend::oir::owned) fn vector_try_reserve(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        additional: i32,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate_vector(value, element)?;
        if additional < 0 {
            return Err(Failure::Bounds);
        }
        let Some(required) = value.length.checked_add(additional) else {
            return Ok(GrowStatus::CapacityOverflow);
        };
        self.vector_change(value, element, required, Mutation::Reserve, fuel)
    }
    pub(in crate::frontend::oir::owned) fn vector_try_push(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        scalar: &[u8],
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        if scalar.len() != element.size() {
            return Err(Failure::Descriptor);
        }
        self.vector_try_extend(value, element, scalar, fuel)
    }
    pub(in crate::frontend::oir::owned) fn vector_try_extend(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        values: &[u8],
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate_vector(value, element)?;
        if !values.len().is_multiple_of(element.size()) {
            return Err(Failure::Descriptor);
        }
        let Some(required) = i32::try_from(values.len() / element.size())
            .ok()
            .and_then(|count| value.length.checked_add(count))
        else {
            return Ok(GrowStatus::CapacityOverflow);
        };
        self.vector_change(value, element, required, Mutation::Extend(values), fuel)
    }
    pub(in crate::frontend::oir::owned) fn vector_try_extend_vector(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        source: &VectorDescriptor,
        start: i32,
        end: i32,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate_vector(value, element)?;
        let bytes = self.vector_range(source, element, start, end)?;
        let pointer = bytes.as_ptr();
        let length = bytes.len();
        if value.identity != 0 && value.identity == source.identity {
            return Err(Failure::Descriptor);
        }
        // SAFETY: source is a distinct live allocation. Only target payload and
        // registry links change; source payload remains allocated and immutable.
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        self.vector_try_extend(value, element, bytes, fuel)
    }
    pub(in crate::frontend::oir::owned) fn vector_try_resize(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        length: i32,
        scalar: &[u8],
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate_vector(value, element)?;
        if length < 0 {
            return Err(Failure::Bounds);
        }
        if scalar.len() != element.size() {
            return Err(Failure::Descriptor);
        }
        self.vector_change(value, element, length, Mutation::Resize(scalar), fuel)
    }
    pub(in crate::frontend::oir::owned) fn vector_truncate(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        length: i32,
    ) -> Result<()> {
        let node = self.locate_vector(value, element)?;
        if length < 0 {
            return Err(Failure::Bounds);
        }
        if length < value.length {
            if let Some(mut node) = node {
                unsafe {
                    node.as_mut().initialized = length as u32;
                }
            }
            value.length = length;
        }
        Ok(())
    }
    pub(in crate::frontend::oir::owned) fn vector_release(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
    ) -> Result<()> {
        if let Some(node) = self.locate_vector(value, element)? {
            self.unlink(node);
        }
        *value = VectorDescriptor::empty();
        Ok(())
    }
    pub(in crate::frontend::oir::owned) fn vector_take(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
    ) -> Result<VectorDescriptor> {
        self.locate_vector(value, element)?;
        Ok(std::mem::replace(value, VectorDescriptor::empty()))
    }
    /// Move into a prepared empty destination. Replacement of a live owner must
    /// use the consumer's complete preflight/drop/commit protocol instead.
    pub(in crate::frontend::oir::owned) fn vector_move(
        &mut self,
        source: &mut VectorDescriptor,
        element: impl VectorType,
        destination: &mut VectorDescriptor,
    ) -> Result<()> {
        self.locate_vector(source, element)?;
        self.locate_vector(destination, element)?;
        if destination.identity != 0 {
            return Err(Failure::Descriptor);
        }
        *destination = std::mem::replace(source, VectorDescriptor::empty());
        Ok(())
    }
    fn vector_change(
        &mut self,
        value: &mut VectorDescriptor,
        element: impl VectorType,
        required: i32,
        mutation: Mutation<'_>,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        let old = self.locate_vector(value, element)?;
        let capacity = super::growth_capacity(value.capacity, required)?;
        let Some(layout) = vector_layout(element, capacity) else {
            return Ok(GrowStatus::CapacityOverflow);
        };
        let grows = capacity != value.capacity;
        let new_length = match mutation {
            Mutation::Reserve => value.length,
            _ => required,
        };
        let writes = new_length.saturating_sub(value.length).max(0) as usize * element.size();
        let scratch = match mutation {
            Mutation::Reserve => &[][..],
            Mutation::Extend(bytes) | Mutation::Resize(bytes) => bytes,
        };
        let copy_bytes = if grows {
            value.length as usize * element.size()
        } else {
            0
        };
        let work = element
            .validation_work(scratch.len())
            .and_then(|work| work.checked_add(writes))
            .and_then(|work| work.checked_add(copy_bytes))
            .ok_or(Failure::Fuel)?;
        if overlaps(
            scratch.as_ptr() as usize,
            scratch.len(),
            value as *const _ as usize,
            16,
        ) || overlaps(
            scratch.as_ptr() as usize,
            scratch.len(),
            fuel as *const _ as usize,
            std::mem::size_of::<usize>(),
        ) || overlaps(
            fuel as *const _ as usize,
            std::mem::size_of::<usize>(),
            self as *const _ as usize,
            std::mem::size_of::<Heap>(),
        ) || old.is_some_and(|node| {
            let charge = unsafe { node_charge(node.as_ref()) } as usize;
            overlaps(
                scratch.as_ptr() as usize,
                scratch.len(),
                node.as_ptr() as usize,
                charge,
            ) || overlaps(
                fuel as *const _ as usize,
                std::mem::size_of::<usize>(),
                node.as_ptr() as usize,
                charge,
            )
        }) {
            return Err(Failure::Descriptor);
        }
        let remaining = fuel.checked_sub(work).ok_or(Failure::Fuel)?;
        if !element.canonical(scratch) {
            return Err(Failure::Descriptor);
        }
        if grows
            && self
                .ledger
                .live_bytes
                .checked_add(layout.size() as u64)
                .is_none_or(|bytes| bytes > self.limit)
        {
            return Ok(GrowStatus::MemoryLimit);
        }
        if grows && self.next_identity == u64::MAX {
            return Err(Failure::IdentityExhausted);
        }
        *fuel = remaining;
        if !grows && new_length == value.length {
            return Ok(GrowStatus::Ok);
        }
        let target = if grows {
            if self.fail(Boundary::Allocate) {
                return Ok(GrowStatus::AllocationFailed);
            }
            let Some(node) = NonNull::new(unsafe { alloc(layout).cast::<VectorHeader>() }) else {
                return Ok(GrowStatus::AllocationFailed);
            };
            unsafe {
                ptr::write(
                    node.as_ptr(),
                    VectorHeader {
                        prefix: Header {
                            next: self.head,
                            previous: ptr::null_mut(),
                            identity: self.next_identity,
                            capacity: VECTOR_NODE,
                            initialized: 0,
                        },
                        capacity: capacity as u32,
                        element: element.tag(),
                        allocation_bytes: layout.size() as u64,
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
            Some(node.cast::<Header>())
        } else {
            old
        };
        if self.fail(Boundary::Commit) {
            if grows {
                self.unlink(target.expect("positive vector growth"));
            }
            return Ok(GrowStatus::AllocationFailed);
        }
        if let Some(mut target) = target {
            // All checks/fault boundaries precede these writes and publication.
            unsafe {
                if grows && copy_bytes != 0 {
                    ptr::copy_nonoverlapping(
                        vector_data(old.expect("initialized source")),
                        vector_data(target),
                        copy_bytes,
                    );
                }
                let tail = vector_data(target).add(value.length as usize * element.size());
                match mutation {
                    Mutation::Reserve => (),
                    Mutation::Extend(bytes) if writes != 0 => {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), tail, writes)
                    }
                    Mutation::Resize(bytes) => {
                        for ordinal in 0..writes / element.size() {
                            ptr::copy_nonoverlapping(
                                bytes.as_ptr(),
                                tail.add(ordinal * element.size()),
                                element.size(),
                            );
                        }
                    }
                    Mutation::Extend(_) => (),
                }
                target.as_mut().initialized = new_length as u32;
                value.identity = target.as_ref().identity;
            }
        }
        value.length = new_length;
        value.capacity = capacity;
        if grows {
            if let Some(old) = old {
                self.unlink(old);
            }
        }
        Ok(GrowStatus::Ok)
    }
}

#[cfg(test)]
#[path = "vector_resources_tests.rs"]
mod tests;
