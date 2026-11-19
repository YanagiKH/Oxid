//! Actual CRT ownership and binary-file behavior on Windows MSVC x64.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

unsafe extern "C" {
    fn _get_osfhandle(fd: i32) -> isize;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetHandleInformation(handle: *mut std::ffi::c_void, flags: *mut u32) -> i32;
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-file-資料-😀-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root.join("bytes-資料-😀.bin"))
    }
    fn bytes(&self) -> &[u8] {
        self.0.to_str().unwrap().as_bytes()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn opened(heap: &mut Heap, path: &[u8], create: bool) -> FileDescriptor {
    match heap.file_open(path, create, &mut usize::MAX).unwrap() {
        OpenValue::Opened(file) => file,
        other => panic!(
            "unexpected open result: {}",
            match other {
                OpenValue::Error(code) => code,
                _ => -1,
            }
        ),
    }
}

#[test]
fn windows_crt_roundtrip_preserves_binary_bytes_and_descriptor_classes() {
    let fixture = Fixture::new();
    let mut heap = Heap::default();
    let mut file = opened(&mut heap, fixture.bytes(), true);
    let fd = heap.file_fd(&file).unwrap().unwrap();
    let mut flags = u32::MAX;
    // The CRT's pointer-width handle is observed only by this host test. The
    // language registry retains fd32 and never stores or truncates that handle.
    let handle = unsafe { _get_osfhandle(fd) } as *mut std::ffi::c_void;
    assert_ne!(unsafe { GetHandleInformation(handle, &mut flags) }, 0);
    assert_eq!(flags & 1, 0, "File descriptors must not be inherited");
    let bytes: Vec<u8> = (0..=255).collect();
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .write(&bytes, &mut usize::MAX)
            .unwrap(),
        Ok(256)
    );
    assert_eq!(heap.file_sync(&file).unwrap(), Ok(()));
    let stale = file.encoding();
    assert!(heap.descriptor(&stale).is_err());
    assert_eq!(heap.file_close(&mut file).unwrap(), Ok(()));
    assert_eq!(heap.file_close(&mut file).unwrap(), Ok(()));
    assert!(heap.file_descriptor(&stale).is_err());
    assert_eq!(fs::read(&fixture.0).unwrap(), bytes);
    file = opened(&mut heap, fixture.bytes(), false);
    let mut buffer = [0xcc; 8];
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .read(&mut buffer[..3], &mut usize::MAX)
            .unwrap(),
        Ok(3)
    );
    assert_eq!(&buffer[..3], &bytes[..3]);
    assert_eq!(&buffer[3..], &[0xcc; 5]);
    assert_eq!(heap.file_seek_absolute(&file, -1).unwrap(), Err(22));
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .read(&mut buffer[..1], &mut usize::MAX)
            .unwrap(),
        Ok(1)
    );
    assert_eq!(buffer[0], 3);
    let mut tail = [0xcc; 256];
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .read(&mut tail, &mut usize::MAX)
            .unwrap(),
        Ok(252)
    );
    assert_eq!(&tail[..252], &bytes[4..]);
    assert_eq!(&tail[252..], &[0xcc; 4]);
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .write(b"x", &mut usize::MAX)
            .unwrap(),
        Err(9)
    );
    assert_eq!(heap.file_seek_absolute(&file, 256).unwrap(), Ok(()));
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .read(&mut buffer, &mut usize::MAX)
            .unwrap(),
        Ok(0)
    );
    assert_eq!(heap.file_close(&mut file).unwrap(), Ok(()));
    assert_eq!(heap.file_sync(&file).unwrap(), Err(9));
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .read(&mut buffer, &mut usize::MAX)
            .unwrap(),
        Err(9)
    );
    assert_eq!(heap.file_seek_absolute(&file, 0).unwrap(), Err(9));
    assert_eq!(heap.ledger.live_bytes, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
}

#[test]
fn windows_crt_allocation_path_and_fuel_failures_precede_truncation() {
    let fixture = Fixture::new();
    fs::write(&fixture.0, b"retained").unwrap();
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        let mut heap = Heap {
            fault: Some((boundary, 0)),
            ..Default::default()
        };
        assert!(matches!(
            heap.file_open(fixture.bytes(), true, &mut usize::MAX)
                .unwrap(),
            OpenValue::AllocationFailed
        ));
        assert_eq!(fs::read(&fixture.0).unwrap(), b"retained");
        assert_eq!(heap.ledger.live_bytes, 0);
        assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    }
    let mut heap = Heap::default();
    assert!(matches!(
        heap.file_open(&[], false, &mut usize::MAX).unwrap(),
        OpenValue::Error(2)
    ));
    let mut missing = fixture.bytes().to_vec();
    missing.extend_from_slice(b".missing");
    assert!(matches!(
        heap.file_open(&missing, false, &mut usize::MAX).unwrap(),
        OpenValue::Error(2)
    ));
    let cost = fixture.bytes().len() * 2 + 1;
    assert_eq!(
        heap.file_open(fixture.bytes(), true, &mut (cost - 1)).err(),
        Some(Failure::Fuel)
    );
    for suffix in [b"\0ignored".as_slice(), b"\xc0\xaf".as_slice()] {
        let mut path = fixture.bytes().to_vec();
        path.extend_from_slice(suffix);
        assert!(matches!(
            heap.file_open(&path, true, &mut usize::MAX).unwrap(),
            OpenValue::Error(22)
        ));
        assert_eq!(fs::read(&fixture.0).unwrap(), b"retained");
    }
    assert!(matches!(
        heap.file_open(&[b'x'; PATH_BYTES], true, &mut 0).unwrap(),
        OpenValue::Error(38)
    ));
    assert_eq!(heap.ledger.live_bytes, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    let file = opened(&mut heap, fixture.bytes(), false);
    assert!(heap.file_descriptor(&file.encoding()).is_ok());
    heap.cleanup();
    assert!(heap.file_descriptor(&file.encoding()).is_err());
    assert_eq!(heap.ledger.live_bytes, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    fs::remove_file(&fixture.0).unwrap();
}

#[test]
fn windows_file_vm_moves_faults_and_fuel_exits_release_the_real_resource() {
    use crate::frontend::{
        build_config::ExecutionPolicy,
        oir::{
            owned::{byte_buffer, execute, source::resource_fixtures::checked_enum_project},
            Scalar,
        },
    };
    let fixture = Fixture::new();
    let path = fixture
        .0
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let text = format!("use std::fs::OpenResult;use std::fs::create;fn main()->i32{{let path=b\"{path}\";let opened=create(&path);match opened{{OpenResult::Opened(file)=>{{let moved=file;return 42;}},OpenResult::Error(code)=>{{return code;}},OpenResult::MemoryLimit=>{{return -1;}},OpenResult::AllocationFailed=>{{return -2;}},}}}}");
    let checked = checked_enum_project(&text);
    let limits = execute::Limits::default();
    let ((result, remaining), observed) = byte_buffer::testing::capture(Default::default(), || {
        execute::observe_witness(
            &checked.witness,
            Some(checked.entry),
            limits,
            ExecutionPolicy::Bounded,
        )
    });
    assert_eq!(result, Ok(Scalar::I32(42)));
    assert_eq!(observed.before_cleanup.allocations, 1);
    assert_eq!(observed.before_cleanup.releases, 1);
    assert_eq!(observed.before_cleanup, observed.after_cleanup);
    let consumed = limits.fuel - remaining;
    for (fuel, success) in [
        (consumed, true),
        (consumed - 1, false),
        (consumed / 2, false),
    ] {
        let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
            execute::observe_witness(
                &checked.witness,
                Some(checked.entry),
                execute::Limits { fuel, ..limits },
                ExecutionPolicy::Bounded,
            )
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(observed.after_cleanup.live_bytes, 0);
        assert_eq!(
            observed.after_cleanup.allocations,
            observed.after_cleanup.releases
        );
    }
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        fs::write(&fixture.0, b"retained").unwrap();
        let control = byte_buffer::testing::Control {
            fault: Some((boundary, 0)),
            ..Default::default()
        };
        let ((result, _), observed) = byte_buffer::testing::capture(control, || {
            execute::observe_witness(
                &checked.witness,
                Some(checked.entry),
                limits,
                ExecutionPolicy::Bounded,
            )
        });
        assert_ne!(result, Ok(Scalar::I32(42)));
        assert!(observed.fault_applied);
        assert_eq!(fs::read(&fixture.0).unwrap(), b"retained");
        assert_eq!(observed.after_cleanup.live_allocations, 0);
        assert_eq!(
            observed.after_cleanup.allocations,
            observed.after_cleanup.releases
        );
    }
    let trapped = checked_enum_project(&text.replace(
        "let moved=file;return 42;",
        "let moved=file;let zero=0;return 1/zero;",
    ));
    let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
        execute::observe_witness(
            &trapped.witness,
            Some(trapped.entry),
            limits,
            ExecutionPolicy::Bounded,
        )
    });
    assert!(result.is_err());
    assert_eq!(observed.after_cleanup.live_bytes, 0);
    assert_eq!(
        observed.after_cleanup.allocations,
        observed.after_cleanup.releases
    );
    fs::remove_file(&fixture.0).unwrap();
}
