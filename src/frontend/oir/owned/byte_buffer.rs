//! Private dynamic byte storage for verified experimental resource operations.
//!
//! Each fallible allocation is one 32-byte intrusive registry header followed
//! by exactly the requested capacity. No Vec/HashMap or separately allocated
//! registry is involved. The normalized limit is not an RSS bound.
use std::{
    alloc::{alloc, dealloc, Layout},
    ptr::{self, NonNull},
    slice,
};

#[path = "file_resources.rs"]
mod files;
#[path = "owned_vector_resources.rs"]
mod owned_vectors;
#[path = "vector_resources.rs"]
mod vectors;
pub(super) use files::{FileAccess, FileDescriptor, OpenValue};
pub(in crate::frontend::oir::owned) use owned_vectors::control_bytes as owned_vector_control_bytes;
pub(in crate::frontend::oir::owned) use owned_vectors::OwnedVectorDescriptor;
pub(in crate::frontend::oir::owned) use vectors::{
    CheckedElement as CheckedVectorElement, Element as VectorElement,
    RecordElement as RecordVectorElement, VectorDescriptor,
};
pub(super) const fn file_helper_control_bytes() -> usize {
    files::helper_control_bytes()
}
pub(super) const fn file_seek_helper_control_bytes() -> usize {
    files::seek_helper_control_bytes()
}

pub(super) const LIMIT: u64 = crate::frontend::build_config::ExecutionPolicy::DEFAULT_HEAP_LIMIT;
pub(super) const ALLOCATION_CHARGE: u64 = 32;

/// Move-only host carrier. Its identity is private to a single execution.
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub(super) struct Descriptor {
    identity: u64,
    length: i32,
    capacity: i32,
}
impl Descriptor {
    pub(super) const fn empty() -> Self {
        Self {
            identity: 0,
            length: 0,
            capacity: 0,
        }
    }

    pub(super) fn encoding(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&self.identity.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.length.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.capacity.to_le_bytes());
        bytes
    }

    /// Private activation encoding. The caller must separately establish the
    /// resource leaf and ownership state from the verified program.
    pub(super) fn write(&self, bytes: &mut [u8]) -> Result<()> {
        let bytes = bytes.get_mut(..16).ok_or(Failure::Descriptor)?;
        bytes.copy_from_slice(&self.encoding());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub(super) enum GrowStatus {
    Ok,
    CapacityOverflow,
    AllocationFailed,
    MemoryLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Bounds,
    Fuel,
    Descriptor,
    IdentityExhausted,
}
type Result<T> = std::result::Result<T, Failure>;

#[repr(C)]
struct Header {
    next: *mut Header,
    previous: *mut Header,
    identity: u64,
    capacity: u32,
    initialized: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Ledger {
    pub live_bytes: u64,
    pub live_allocations: u64,
    pub peak_bytes: u64,
    pub allocations: u64,
    pub releases: u64,
}

/// One execution owns this registry. Drop is the fatal-error cleanup backstop;
/// ordinary verified owner exits must explicitly release their descriptors.
pub(super) struct Heap {
    head: *mut Header,
    next_identity: u64,
    limit: u64,
    ledger: Ledger,
    #[cfg(test)]
    fault: Option<(Boundary, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Boundary {
    Allocate,
    Initialize,
    Commit,
}

const _: () = {
    assert!(std::mem::size_of::<Descriptor>() == 16);
    assert!(std::mem::align_of::<Descriptor>() == 8);
    // This experiment's supported execution host is Linux x86_64.
    #[cfg(target_pointer_width = "64")]
    assert!(std::mem::size_of::<Header>() == ALLOCATION_CHARGE as usize);
    #[cfg(all(not(test), target_pointer_width = "64"))]
    assert!(std::mem::size_of::<Heap>() == 64);
};

/// Pure capacity arithmetic, suitable for boundary tests without large heaps.
pub(super) fn growth_capacity(current: i32, required: i32) -> Result<i32> {
    if current < 0 || required < 0 {
        return Err(Failure::Bounds);
    }
    if required <= current {
        return Ok(current);
    }
    Ok(required.max(current.saturating_mul(2).max(8)))
}

impl Default for Heap {
    fn default() -> Self {
        Self {
            head: ptr::null_mut(),
            next_identity: 1,
            limit: LIMIT,
            ledger: Ledger::default(),
            #[cfg(test)]
            fault: None,
        }
    }
}

impl Heap {
    pub(super) fn for_execution(limit: u64) -> Self {
        #[cfg(test)]
        {
            testing::heap(limit)
        }
        #[cfg(not(test))]
        {
            Self {
                limit,
                ..Self::default()
            }
        }
    }
    /// Decoding an activation does not grant allocation authority: membership
    /// and initialized extent are checked against this execution's registry.
    pub(super) fn descriptor(&self, bytes: &[u8]) -> Result<Descriptor> {
        let bytes = bytes.get(..16).ok_or(Failure::Descriptor)?;
        let value = Descriptor {
            identity: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            length: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            capacity: i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        };
        self.locate(&value)?;
        Ok(value)
    }

    pub(super) fn ledger(&self) -> Ledger {
        self.ledger
    }

    /// Validate identity by searching live nodes before dereferencing any
    /// descriptor-derived address. IDs are monotonic and never reused here.
    fn locate(&self, value: &Descriptor) -> Result<Option<NonNull<Header>>> {
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
            // SAFETY: Only our allocation/link/unlink routines change the list;
            // every node remains allocated until unlinked under &mut self.
            let header = unsafe { node.as_ref() };
            if header.identity == value.identity {
                return if header.capacity == value.capacity as u32
                    && header.initialized == value.length as u32
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

    pub(super) fn len(&self, value: &Descriptor) -> Result<i32> {
        self.locate(value)?;
        Ok(value.length)
    }
    pub(super) fn capacity(&self, value: &Descriptor) -> Result<i32> {
        self.locate(value)?;
        Ok(value.capacity)
    }
    pub(super) fn bytes<'a>(&'a self, value: &Descriptor) -> Result<&'a [u8]> {
        let node = self.locate(value)?;
        let Some(node) = node else {
            return Ok(&[]);
        };
        // SAFETY: locate checks the registry's initialized extent. &self keeps
        // the allocation alive and prevents mutation for the returned lifetime.
        Ok(unsafe { slice::from_raw_parts(data(node), value.length as usize) })
    }
    pub(super) fn range<'a>(
        &'a self,
        value: &Descriptor,
        start: i32,
        end: i32,
    ) -> Result<&'a [u8]> {
        let bytes = self.bytes(value)?;
        bounds(start, end, value.length)?;
        Ok(&bytes[start as usize..end as usize])
    }
    pub(super) fn range_mut<'a>(
        &'a mut self,
        value: &Descriptor,
        start: i32,
        end: i32,
    ) -> Result<&'a mut [u8]> {
        let node = self.locate(value)?;
        bounds(start, end, value.length)?;
        let Some(node) = node else {
            return Ok(&mut []);
        };
        // SAFETY: &mut self uniquely loans all heap storage; bounds and locate
        // prove the complete requested extent is initialized and live.
        Ok(unsafe {
            slice::from_raw_parts_mut(data(node).add(start as usize), (end - start) as usize)
        })
    }

    pub(super) fn try_reserve(
        &mut self,
        value: &mut Descriptor,
        additional: i32,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate(value)?;
        if additional < 0 {
            return Err(Failure::Bounds);
        }
        let Some(required) = value.length.checked_add(additional) else {
            return Ok(GrowStatus::CapacityOverflow);
        };
        self.change(value, required, Mutation::Reserve, fuel)
    }
    pub(super) fn try_push(
        &mut self,
        value: &mut Descriptor,
        byte: u8,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.try_extend(value, &[byte], fuel)
    }
    pub(super) fn try_extend(
        &mut self,
        value: &mut Descriptor,
        source: &[u8],
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate(value)?;
        let Some(required) = i32::try_from(source.len())
            .ok()
            .and_then(|length| value.length.checked_add(length))
        else {
            return Ok(GrowStatus::CapacityOverflow);
        };
        self.change(value, required, Mutation::Extend(source), fuel)
    }
    /// Both descriptors have execution-private checked identities. This is a
    /// borrow of another live allocation, never an implicit clone or owner copy.
    pub(super) fn try_extend_buffer(
        &mut self,
        value: &mut Descriptor,
        source: &Descriptor,
        start: i32,
        end: i32,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate(value)?;
        let bytes = self.range(source, start, end)?;
        let pointer = bytes.as_ptr();
        let length = bytes.len();
        if value.identity != 0 && value.identity == source.identity {
            return Err(Failure::Descriptor);
        }
        // SAFETY: range checked source membership and initialized extent. Distinct
        // identities imply disjoint allocations; growth only replaces value's
        // allocation. Registry link writes touch headers, never source payload.
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        self.try_extend(value, bytes, fuel)
    }

    pub(super) fn try_resize(
        &mut self,
        value: &mut Descriptor,
        length: i32,
        fill: u8,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        self.locate(value)?;
        if length < 0 {
            return Err(Failure::Bounds);
        }
        self.change(value, length, Mutation::Resize(fill), fuel)
    }
    pub(super) fn truncate(&mut self, value: &mut Descriptor, length: i32) -> Result<()> {
        let node = self.locate(value)?;
        if length < 0 {
            return Err(Failure::Bounds);
        }
        value.length = value.length.min(length);
        if let Some(mut node) = node {
            // SAFETY: Validated, exclusively borrowed live registry node.
            unsafe {
                node.as_mut().initialized = value.length as u32;
            }
        }
        Ok(())
    }

    fn change(
        &mut self,
        value: &mut Descriptor,
        required: i32,
        mutation: Mutation<'_>,
        fuel: &mut usize,
    ) -> Result<GrowStatus> {
        let old = self.locate(value)?;
        let capacity = growth_capacity(value.capacity, required)?;
        let grows = capacity != value.capacity;
        let new_length = match mutation {
            Mutation::Reserve => value.length,
            _ => required,
        };
        let writes = match mutation {
            Mutation::Reserve => 0,
            _ => new_length.saturating_sub(value.length).max(0) as usize,
        };
        if !grows && new_length == value.length {
            return Ok(GrowStatus::Ok);
        }
        let charge = capacity as u64 + ALLOCATION_CHARGE;
        if grows
            && self
                .ledger
                .live_bytes
                .checked_add(charge)
                .is_none_or(|v| v > self.limit)
        {
            return Ok(GrowStatus::MemoryLimit);
        }
        if grows && self.next_identity == u64::MAX {
            return Err(Failure::IdentityExhausted);
        }
        // Descriptor work belongs to the consumer's ordinary instruction charge.
        // Dynamic work is separate and prepaid even if allocation later fails.
        let work = writes + if grows { value.length as usize } else { 0 };
        *fuel = fuel.checked_sub(work).ok_or(Failure::Fuel)?;
        let target = if grows {
            if self.fail(Boundary::Allocate) {
                return Ok(GrowStatus::AllocationFailed);
            }
            let layout = allocation_layout(capacity as u32);
            // SAFETY: Layout is nonzero and checked. Null is recoverable. The
            // header and payload are one allocation; no reserve follows it.
            let Some(mut node) = NonNull::new(unsafe { alloc(layout).cast::<Header>() }) else {
                return Ok(GrowStatus::AllocationFailed);
            };
            // SAFETY: Entire header is writable. Linking and accounting cannot
            // fail or allocate; the new node is tracked before any later step.
            unsafe {
                ptr::write(
                    node.as_ptr(),
                    Header {
                        next: self.head,
                        previous: ptr::null_mut(),
                        identity: self.next_identity,
                        capacity: capacity as u32,
                        initialized: 0,
                    },
                );
                if !self.head.is_null() {
                    (*self.head).previous = node.as_ptr();
                }
                self.head = node.as_ptr();
                node.as_mut().previous = ptr::null_mut();
            }
            self.next_identity += 1;
            self.ledger.live_bytes += charge;
            self.ledger.live_allocations += 1;
            self.ledger.allocations += 1;
            self.ledger.peak_bytes = self.ledger.peak_bytes.max(self.ledger.live_bytes);
            if self.fail(Boundary::Initialize) {
                self.unlink(node);
                return Ok(GrowStatus::AllocationFailed);
            }
            Some(node)
        } else {
            old
        };
        if self.fail(Boundary::Commit) {
            if grows {
                self.unlink(target.expect("positive growth"));
            }
            return Ok(GrowStatus::AllocationFailed);
        }
        if let Some(mut target) = target {
            // SAFETY: No fallible step remains. New and old allocations do not
            // overlap. Safe callers cannot alias source bytes with &mut Heap.
            // Only [0,new_length) is copied/initialized; spare capacity is unread.
            unsafe {
                if grows {
                    if let Some(old) = old {
                        ptr::copy_nonoverlapping(data(old), data(target), value.length as usize);
                    }
                }
                if writes != 0 {
                    let tail = data(target).add(value.length as usize);
                    match mutation {
                        Mutation::Extend(bytes) => {
                            ptr::copy_nonoverlapping(bytes.as_ptr(), tail, writes)
                        }
                        Mutation::Resize(fill) => ptr::write_bytes(tail, fill, writes),
                        Mutation::Reserve => unreachable!(),
                    }
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

    /// The verified consumer must call this only for initialized resource
    /// leaves. Validation can report a malformed descriptor; physical release
    /// after validation is infallible, allocation-free, and does not use fuel.
    pub(super) fn release(&mut self, value: &mut Descriptor) -> Result<()> {
        if let Some(node) = self.locate(value)? {
            self.unlink(node);
        }
        *value = Descriptor::empty();
        Ok(())
    }
    fn unlink(&mut self, node: NonNull<Header>) {
        // SAFETY: All callers provide a live node from this registry, exactly
        // once. Copy its links and size before deallocating its entire block.
        unsafe {
            let header = node.as_ref();
            let vector = header.capacity == vectors::VECTOR_NODE;
            let owned_vector = header.capacity == owned_vectors::OWNED_VECTOR_NODE;
            let capacity = if header.capacity == files::FILE_NODE {
                let _ = files::close_node(header);
                0
            } else {
                header.capacity
            };
            let charge = if vector {
                vectors::node_charge(header)
            } else if owned_vector {
                owned_vectors::node_charge(header)
            } else {
                capacity as u64 + ALLOCATION_CHARGE
            };
            if header.previous.is_null() {
                self.head = header.next;
            } else {
                (*header.previous).next = header.next;
            }
            if !header.next.is_null() {
                (*header.next).previous = header.previous;
            }
            if vector {
                vectors::deallocate_node(node);
            } else if owned_vector {
                owned_vectors::deallocate_node(node);
            } else {
                dealloc(node.as_ptr().cast(), allocation_layout(capacity));
            }
            self.ledger.live_bytes -= charge;
            self.ledger.live_allocations -= 1;
            self.ledger.releases += 1;
        }
    }
    pub(super) fn cleanup(&mut self) {
        while let Some(node) = NonNull::new(self.head) {
            self.unlink(node);
        }
    }
    fn fail(&mut self, boundary: Boundary) -> bool {
        #[cfg(test)]
        if let Some((selected, skip)) = self.fault.as_mut() {
            if *selected == boundary {
                if *skip == 0 {
                    self.fault = None;
                    return true;
                }
                *skip -= 1;
            }
        }
        let _ = boundary;
        false
    }
}
impl Drop for Heap {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
pub(super) mod testing {
    use super::*;
    use std::cell::RefCell;
    #[derive(Clone, Copy, Default)]
    pub(in crate::frontend::oir::owned) struct Control {
        pub limit: Option<u64>,
        pub fault: Option<(Boundary, usize)>,
    }
    #[derive(Clone, Copy, Debug, Default)]
    pub(in crate::frontend::oir::owned) struct Observation {
        pub before_cleanup: Ledger,
        pub after_cleanup: Ledger,
        pub fault_applied: bool,
    }
    thread_local! {
        static ACTIVE: RefCell<Option<(Control, Observation)>> = const { RefCell::new(None) };
    }
    pub(super) fn heap(limit: u64) -> Heap {
        let control = ACTIVE.with(|active| active.borrow().as_ref().map(|(control, _)| *control));
        let mut heap = Heap {
            limit,
            ..Heap::default()
        };
        if let Some(control) = control {
            heap.limit = control.limit.unwrap_or(limit).min(limit);
            heap.fault = control.fault;
        }
        heap
    }
    pub(in crate::frontend::oir::owned) fn record(heap: &Heap, before: Ledger) {
        ACTIVE.with(|active| {
            if let Some((control, observed)) = active.borrow_mut().as_mut() {
                *observed = Observation {
                    before_cleanup: before,
                    after_cleanup: heap.ledger(),
                    fault_applied: control.fault.is_some() && heap.fault.is_none(),
                };
            }
        });
    }
    pub(in crate::frontend::oir::owned) fn capture<T>(
        control: Control,
        run: impl FnOnce() -> T,
    ) -> (T, Observation) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                ACTIVE.with(|active| *active.borrow_mut() = None);
            }
        }
        ACTIVE.with(|active| {
            assert!(active.borrow().is_none());
            *active.borrow_mut() = Some((control, Observation::default()));
        });
        let reset = Reset;
        let value = run();
        let observed = ACTIVE.with(|active| active.borrow().as_ref().unwrap().1);
        drop(reset);
        (value, observed)
    }
}

fn allocation_layout(capacity: u32) -> Layout {
    // i32 capacity plus 32 fits usize/isize on every supported execution host.
    Layout::from_size_align(std::mem::size_of::<Header>() + capacity as usize, 8)
        .expect("bounded byte allocation layout")
}
fn data(node: NonNull<Header>) -> *mut u8 {
    // SAFETY: A header always has a contiguous payload of its admitted capacity.
    unsafe { node.as_ptr().add(1).cast() }
}
fn bounds(start: i32, end: i32, length: i32) -> Result<()> {
    if start < 0 || end < start || end > length {
        Err(Failure::Bounds)
    } else {
        Ok(())
    }
}
enum Mutation<'a> {
    Reserve,
    Extend(&'a [u8]),
    Resize(u8),
}

#[cfg(test)]
mod tests;
