#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use super::*;
use std::{
    fs,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Path(PathBuf);
impl Path {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "oxid-file-resource-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn bytes(&self) -> &[u8] {
        self.0.as_os_str().as_bytes()
    }
}
impl Drop for Path {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn opened(heap: &mut Heap, path: &Path, create: bool) -> FileDescriptor {
    let mut fuel = usize::MAX;
    match heap.file_open(path.bytes(), create, &mut fuel).unwrap() {
        OpenValue::Opened(value) => value,
        _ => panic!("expected an opened file"),
    }
}
#[test]
fn file_registry_tracks_exact_allocations_and_releases_os_descriptors() {
    let mut fuel = usize::MAX;
    let path = Path::new();
    let mut heap = Heap::default();
    let mut output = opened(&mut heap, &path, true);
    let fd = heap.file_fd(&output).unwrap().unwrap();
    assert_eq!(heap.ledger.live_bytes, 32);
    // SAFETY: F_GETFD observes a descriptor owned by this test's private heap.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0 && flags & libc::FD_CLOEXEC != 0);
    let value = b"A\0\xFF";
    assert_eq!(
        heap.file_access(&output)
            .unwrap()
            .write(value, &mut fuel)
            .unwrap(),
        Ok(3)
    );
    assert_eq!(heap.file_sync(&output).unwrap(), Ok(()));
    let stale = output.encoding();
    assert_eq!(heap.file_close(&mut output).unwrap(), Ok(()));
    assert_eq!(output.encoding(), [0; 16]);
    assert_eq!(heap.file_close(&mut output).unwrap(), Ok(()));
    assert!(heap.file_descriptor(&stale).is_err());
    // SAFETY: F_GETFD checks that close released this fd before any reuse.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
    let input = opened(&mut heap, &path, false);
    let mut bytes = [0; 8];
    assert_eq!(
        heap.file_access(&input)
            .unwrap()
            .read(&mut bytes, &mut fuel)
            .unwrap(),
        Ok(3)
    );
    assert_eq!(&bytes[..3], value);
    assert_eq!(
        heap.file_access(&input)
            .unwrap()
            .read(&mut bytes, &mut fuel)
            .unwrap(),
        Ok(0)
    );
    heap.cleanup();
    assert_eq!(heap.ledger.live_bytes, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    assert_eq!(heap.ledger.peak_bytes, 32);
}
#[test]
fn files_and_buffers_cannot_reinterpret_each_others_identities() {
    let mut fuel = usize::MAX;
    let path = Path::new();
    let mut heap = Heap::default();
    let file = opened(&mut heap, &path, true);
    let encoding = file.encoding();
    assert!(heap.descriptor(&encoding).is_err());
    let mut forged = encoding;
    forged[8] = 1;
    assert!(heap.file_descriptor(&forged).is_err());
    let mut buffer = Descriptor::empty();
    assert_eq!(
        heap.try_reserve(&mut buffer, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut forged = buffer.encoding();
    forged[8..].fill(0);
    assert!(heap.file_descriptor(&forged).is_err());
    assert_eq!(heap.ledger.live_allocations, 2);
    heap.cleanup();
    assert_eq!(heap.ledger.live_allocations, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
}
#[test]
fn every_open_allocation_fault_precedes_truncation_and_returns_all_storage() {
    let mut fuel = usize::MAX;
    let path = Path::new();
    fs::write(&path.0, b"retained").unwrap();
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        let mut heap = Heap {
            fault: Some((boundary, 0)),
            ..Default::default()
        };
        assert!(matches!(
            heap.file_open(path.bytes(), true, &mut fuel).unwrap(),
            OpenValue::AllocationFailed
        ));
        assert_eq!(fs::read(&path.0).unwrap(), b"retained");
        assert_eq!(heap.ledger.live_bytes, 0);
        assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    }
    let mut heap = Heap {
        limit: 31,
        ..Default::default()
    };
    assert!(matches!(
        heap.file_open(path.bytes(), true, &mut fuel).unwrap(),
        OpenValue::MemoryLimit
    ));
    assert_eq!(fs::read(&path.0).unwrap(), b"retained");
    assert_eq!(heap.ledger.allocations, 0);
    heap.limit = 32;
    let mut file = opened(&mut heap, &path, true);
    assert_eq!(heap.ledger.live_bytes, 32);
    assert_eq!(heap.file_close(&mut file).unwrap(), Ok(()));
}
#[test]
fn open_validation_and_fuel_refuse_before_path_effects() {
    let mut fuel = usize::MAX;
    let path = Path::new();
    fs::write(&path.0, b"retained").unwrap();
    let mut heap = Heap::default();
    let cost = 2 * path.bytes().len() + 1;
    assert!(matches!(
        heap.file_open(path.bytes(), true, &mut (cost - 1)),
        Err(Failure::Fuel)
    ));
    assert_eq!(fs::read(&path.0).unwrap(), b"retained");
    let mut invalid = path.bytes().to_vec();
    invalid.extend_from_slice(b"\0suffix");
    assert!(matches!(
        heap.file_open(&invalid, true, &mut fuel).unwrap(),
        OpenValue::Error(22)
    ));
    assert_eq!(fs::read(&path.0).unwrap(), b"retained");
    assert!(matches!(
        heap.file_open(&[b'x'; 4096], true, &mut 0).unwrap(),
        OpenValue::Error(36)
    ));
    assert_eq!(heap.ledger.allocations, 0);
}
