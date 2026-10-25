//! File identities in the execution's existing intrusive resource registry.
//! A file is a distinct language resource; byte-buffer descriptors cannot name it.
use super::*;

pub(super) const FILE_NODE: u32 = u32::MAX;
pub(in crate::frontend::oir::owned) const PATH_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub(in crate::frontend::oir::owned) struct FileDescriptor {
    identity: u64,
    reserved: u64,
}
impl FileDescriptor {
    pub(in crate::frontend::oir::owned) fn encoding(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&self.identity.to_le_bytes());
        bytes
    }
}
pub(in crate::frontend::oir::owned) enum OpenValue {
    Opened(FileDescriptor),
    Error(i32),
    MemoryLimit,
    AllocationFailed,
}

/// Synchronous helper family below FileDelivery. Path backing arrays are paid
/// by that caller; registry nodes remain charged to the shared heap ledger.
#[allow(dead_code)]
struct HelperCarriers {
    heap_argument: *mut Heap,
    descriptor_arguments: [&'static FileDescriptor; 3],
    byte_argument: &'static [u8],
    fuel_argument: &'static mut usize,
    decoded_descriptor: FileDescriptor,
    descriptor_return: Result<FileDescriptor>,
    locate_return: Result<Option<NonNull<Header>>>,
    selected_node: Option<NonNull<Header>>,
    active_node: NonNull<Header>,
    cursor: *mut Header,
    selected_header: &'static Header,
    file_fd_return: Result<Option<i32>>,
    selected_fd: Option<i32>,
    access: FileAccess,
    access_return: Result<FileAccess>,
    raw_open_return: Result<OpenValue>,
    raw_attempt_return: Result<std::result::Result<usize, i32>>,
    raw_status_return: Result<std::result::Result<(), i32>>,
    close_node_return: std::result::Result<(), i32>,
    os_error: std::io::Error,
    os_error_code: Option<i32>,
    syscall_count: isize,
    syscall_fd: i32,
    syscall_flags: i32,
    allocation_layout: Layout,
    allocation: Option<NonNull<Header>>,
    allocation_identity: u64,
    work_products: [Option<usize>; 2],
    work_return: Result<usize>,
    ledger_sum: Option<u64>,
    encoded_descriptor: [u8; 16],
    descriptor_slice_return: Result<&'static [u8]>,
    descriptor_slice_option: Option<&'static [u8]>,
    descriptor_slice: &'static [u8],
    descriptor_words_return: [std::result::Result<[u8; 8], std::array::TryFromSliceError>; 2],
    descriptor_words: [[u8; 8]; 2],
}
pub(super) const fn helper_control_bytes() -> usize {
    size_of::<HelperCarriers>()
}

impl Heap {
    fn locate_file(&self, value: &FileDescriptor) -> Result<Option<NonNull<Header>>> {
        if value.reserved != 0 {
            return Err(Failure::Descriptor);
        }
        if value.identity == 0 {
            return Ok(None);
        }
        let mut cursor = self.head;
        while let Some(node) = NonNull::new(cursor) {
            // SAFETY: Only this execution links and releases these live nodes.
            let header = unsafe { node.as_ref() };
            if header.identity == value.identity {
                return if header.capacity == FILE_NODE && header.initialized <= i32::MAX as u32 {
                    Ok(Some(node))
                } else {
                    Err(Failure::Descriptor)
                };
            }
            cursor = header.next;
        }
        Err(Failure::Descriptor)
    }
    pub(in crate::frontend::oir::owned) fn file_descriptor(
        &self,
        bytes: &[u8],
    ) -> Result<FileDescriptor> {
        let bytes = bytes.get(..16).ok_or(Failure::Descriptor)?;
        let value = FileDescriptor {
            identity: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            reserved: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        };
        self.locate_file(&value)?;
        Ok(value)
    }
    pub(in crate::frontend::oir::owned) fn file_open(
        &mut self,
        path: &[u8],
        create: bool,
        fuel: &mut usize,
    ) -> Result<OpenValue> {
        if path.len() >= PATH_BYTES {
            return Ok(OpenValue::Error(36));
        }
        let work = path
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Failure::Fuel)?;
        *fuel = fuel.checked_sub(work).ok_or(Failure::Fuel)?;
        if path.contains(&0) {
            return Ok(OpenValue::Error(22));
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = create;
            return Ok(OpenValue::Error(38));
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            let mut terminated = [0u8; PATH_BYTES];
            terminated[..path.len()].copy_from_slice(path);
            if self
                .ledger
                .live_bytes
                .checked_add(ALLOCATION_CHARGE)
                .is_none_or(|n| n > self.limit)
            {
                return Ok(OpenValue::MemoryLimit);
            }
            if self.next_identity == u64::MAX {
                return Err(Failure::IdentityExhausted);
            }
            if self.fail(Boundary::Allocate) {
                return Ok(OpenValue::AllocationFailed);
            }
            // SAFETY: Header-only allocation has a nonzero, checked layout.
            let Some(node) = NonNull::new(unsafe { alloc(allocation_layout(0)).cast::<Header>() })
            else {
                return Ok(OpenValue::AllocationFailed);
            };
            let identity = self.next_identity;
            // Register the pending allocation before any subsequent failure;
            // its closed marker makes fatal cleanup safe before open succeeds.
            // SAFETY: The fresh header and the exclusively held list are writable.
            unsafe {
                ptr::write(
                    node.as_ptr(),
                    Header {
                        next: self.head,
                        previous: ptr::null_mut(),
                        identity,
                        capacity: FILE_NODE,
                        initialized: u32::MAX,
                    },
                );
                if !self.head.is_null() {
                    (*self.head).previous = node.as_ptr();
                }
            }
            self.head = node.as_ptr();
            self.next_identity += 1;
            self.ledger.live_bytes += ALLOCATION_CHARGE;
            self.ledger.live_allocations += 1;
            self.ledger.allocations += 1;
            self.ledger.peak_bytes = self.ledger.peak_bytes.max(self.ledger.live_bytes);
            // Test refusals must happen before open(O_TRUNC) can alter a file.
            if self.fail(Boundary::Initialize) || self.fail(Boundary::Commit) {
                self.unlink(node);
                return Ok(OpenValue::AllocationFailed);
            }
            let flags = libc::O_CLOEXEC
                | libc::O_NOCTTY
                | if create {
                    libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
                } else {
                    libc::O_RDONLY
                };
            // SAFETY: The bounded path is NUL terminated with no interior NUL.
            let fd =
                unsafe { libc::open(terminated.as_ptr().cast(), flags, 0o600 as libc::mode_t) };
            if fd < 0 {
                let error = errno();
                self.unlink(node);
                return Ok(OpenValue::Error(error));
            }
            // No fallible operation remains between open and ownership commit.
            // SAFETY: This execution owns the live pending registry header.
            unsafe {
                (*node.as_ptr()).initialized = fd as u32;
            }
            Ok(OpenValue::Opened(FileDescriptor {
                identity,
                reserved: 0,
            }))
        }
    }
    fn file_fd(&self, value: &FileDescriptor) -> Result<Option<i32>> {
        Ok(self.locate_file(value)?.map(|node| {
            // SAFETY: locate_file establishes live file membership.
            unsafe { node.as_ref().initialized as i32 }
        }))
    }
    pub(in crate::frontend::oir::owned) fn file_access(
        &self,
        value: &FileDescriptor,
    ) -> Result<FileAccess> {
        Ok(FileAccess {
            fd: self.file_fd(value)?,
        })
    }
    pub(in crate::frontend::oir::owned) fn file_sync(
        &self,
        value: &FileDescriptor,
    ) -> Result<std::result::Result<(), i32>> {
        let Some(fd) = self.file_fd(value)? else {
            return Ok(Err(9));
        };
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            // SAFETY: The live registry exclusively owns this OS descriptor.
            Ok(if unsafe { libc::fsync(fd) } < 0 {
                Err(errno())
            } else {
                Ok(())
            })
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = fd;
            Ok(Err(38))
        }
    }
    pub(in crate::frontend::oir::owned) fn file_close(
        &mut self,
        value: &mut FileDescriptor,
    ) -> Result<std::result::Result<(), i32>> {
        let Some(mut node) = self.locate_file(value)? else {
            return Ok(Ok(()));
        };
        // SAFETY: Only this registry can close the live descriptor. Linux close
        // is never retried after EINTR; the descriptor has already been freed.
        let result = unsafe { close_node(node.as_ref()) };
        // Mark it closed before unlink, so cleanup cannot close a reused fd.
        unsafe {
            node.as_mut().initialized = u32::MAX;
        }
        self.unlink(node);
        value.identity = 0;
        Ok(result)
    }
}
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(5)
}

/// The caller owns a live registry header and will unlink it exactly once.
pub(super) unsafe fn close_node(header: &Header) -> std::result::Result<(), i32> {
    if header.initialized == u32::MAX {
        return Ok(());
    }
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        // SAFETY: The registry owns this descriptor; no retry is performed.
        if unsafe { libc::close(header.initialized as i32) } < 0 {
            Err(errno())
        } else {
            Ok(())
        }
    }
    #[cfg(not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        Err(38)
    }
}

/// A short-lived capability validated before the byte loan becomes active.
pub(in crate::frontend::oir::owned) struct FileAccess {
    fd: Option<i32>,
}
impl FileAccess {
    pub(in crate::frontend::oir::owned) fn read(
        self,
        bytes: &mut [u8],
        fuel: &mut usize,
    ) -> Result<std::result::Result<usize, i32>> {
        let fd = self.fd;
        *fuel = fuel.checked_sub(bytes.len()).ok_or(Failure::Fuel)?;
        let Some(fd) = fd else {
            return Ok(Err(9));
        };
        if bytes.is_empty() {
            return Ok(Ok(0));
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            // SAFETY: The entire uniquely borrowed output extent is writable.
            let count = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
            Ok(if count < 0 {
                Err(errno())
            } else {
                Ok(count as usize)
            })
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = fd;
            Ok(Err(38))
        }
    }
    pub(in crate::frontend::oir::owned) fn write(
        self,
        bytes: &[u8],
        fuel: &mut usize,
    ) -> Result<std::result::Result<usize, i32>> {
        let fd = self.fd;
        *fuel = fuel.checked_sub(bytes.len()).ok_or(Failure::Fuel)?;
        let Some(fd) = fd else {
            return Ok(Err(9));
        };
        if bytes.is_empty() {
            return Ok(Ok(0));
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            // SAFETY: The borrowed input extent is initialized and readable.
            let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            Ok(if count < 0 {
                Err(errno())
            } else {
                Ok(count as usize)
            })
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = fd;
            Ok(Err(38))
        }
    }
}

#[cfg(test)]
#[path = "file_resource_tests.rs"]
mod tests;
