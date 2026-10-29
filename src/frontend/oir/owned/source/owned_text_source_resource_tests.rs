//! Real project and package source calls under the runtime allocation failpoint.
use super::super::super::byte_buffer;
use super::*;
use crate::frontend::{
    build_config::{ExecutionPolicy, HOSTED_FUEL},
    project::{
        packages::{PackageDependency, PackageSources, PackageSpec},
        ProjectLimits, ProjectSources,
    },
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const TEXT_SOURCE: &str = include_str!("../../../../../packages/utf8_text/src/lib.ox");
const IMPORTS: &str = r#"
use alloc::bytes::GrowStatus;
use crate::text::Text;
use crate::text::FromUtf8;
use crate::text::AppendStatus;
use crate::text::new_text;
use crate::text::from_utf8;
use crate::text::append_utf8;
use crate::text::append_text;
use crate::text::byte_len;
use crate::text::byte_capacity;
use crate::text::scalar_count;
use crate::text::equals_utf8;
use crate::text::try_reserve_bytes;

fn appended(status: AppendStatus) -> bool {
    match status {
        AppendStatus::Ok => { return true; },
        AppendStatus::InvalidInput(offset) => { return false; },
        AppendStatus::CapacityOverflow => { return false; },
        AppendStatus::AllocationFailed => { return false; },
        AppendStatus::MemoryLimit => { return false; },
    }
}

fn grew(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

fn unchanged(text: &Text, expected: &[u8], original_capacity: i32) -> bool {
    return byte_len(&*text) == 8 && scalar_count(&*text) == 8
        && byte_capacity(&*text) == original_capacity && equals_utf8(&*text, &*expected);
}
"#;

const FROM_UTF8: &str = r#"
fn main() -> i32 {
    let invalid = [65u8, 255u8];
    let invalid_result = from_utf8(&invalid);
    match invalid_result {
        FromUtf8::InvalidInput(offset) => { if offset != 1 { return 1; } },
        FromUtf8::Ok(text) => { return 2; },
        FromUtf8::CapacityOverflow => { return 2; },
        FromUtf8::AllocationFailed => { return 2; },
        FromUtf8::MemoryLimit => { return 2; },
    }
    let empty: [u8; 0] = [];
    let empty_result = from_utf8(&empty);
    match empty_result {
        FromUtf8::Ok(text) => {
            if byte_len(&text) != 0 || byte_capacity(&text) != 0 || scalar_count(&text) != 0 { return 3; }
        },
        FromUtf8::InvalidInput(offset) => { return 4; },
        FromUtf8::CapacityOverflow => { return 4; },
        FromUtf8::AllocationFailed => { return 4; },
        FromUtf8::MemoryLimit => { return 4; },
    }
    let valid = b"ABCDEFGH";
    let valid_result = from_utf8(&valid);
    match valid_result {
        FromUtf8::AllocationFailed => {},
        FromUtf8::Ok(text) => { return 5; },
        FromUtf8::InvalidInput(offset) => { return 5; },
        FromUtf8::CapacityOverflow => { return 5; },
        FromUtf8::MemoryLimit => { return 5; },
    }
    if invalid[0] != 65u8 || invalid[1] != 255u8 || valid[0] != 65u8 || valid[7] != 72u8 { return 6; }
    return 0;
}
"#;

const APPEND_TEXT: &str = r#"
fn main() -> i32 {
    let expected = b"ABCDEFGH";
    let doubled = b"ABCDEFGHABCDEFGH";
    let mut destination = new_text();
    let mut source = new_text();
    if !appended(append_utf8(&mut destination, &expected)) { return 1; }
    if !appended(append_utf8(&mut source, &expected)) { return 2; }
    let destination_capacity = byte_capacity(&destination);
    let source_capacity = byte_capacity(&source);
    if destination_capacity != 8 || source_capacity != 8 { return 3; }
    let refused = append_text(&mut destination, &source);
    match refused {
        GrowStatus::AllocationFailed => {},
        GrowStatus::Ok => { return 4; },
        GrowStatus::CapacityOverflow => { return 4; },
        GrowStatus::MemoryLimit => { return 4; },
    }
    if !unchanged(&destination, &expected, destination_capacity)
        || !unchanged(&source, &expected, source_capacity) { return 5; }
    if !grew(append_text(&mut destination, &source)) { return 6; }
    if !equals_utf8(&destination, &doubled) || byte_len(&destination) != 16
        || scalar_count(&destination) != 16 || !unchanged(&source, &expected, source_capacity) { return 7; }
    return 0;
}
"#;

const RESERVE_BYTES: &str = r#"
fn main() -> i32 {
    let expected = b"ABCDEFGH";
    let mut text = new_text();
    if !appended(append_utf8(&mut text, &expected)) { return 1; }
    let original_capacity = byte_capacity(&text);
    if original_capacity != 8 { return 2; }
    let refused = try_reserve_bytes(&mut text, 1);
    match refused {
        GrowStatus::AllocationFailed => {},
        GrowStatus::Ok => { return 3; },
        GrowStatus::CapacityOverflow => { return 3; },
        GrowStatus::MemoryLimit => { return 3; },
    }
    if !unchanged(&text, &expected, original_capacity) { return 4; }
    if !grew(try_reserve_bytes(&mut text, 1)) { return 5; }
    if byte_capacity(&text) < 9 || byte_len(&text) != 8 || scalar_count(&text) != 8
        || !equals_utf8(&text, &expected) { return 6; }
    return 0;
}
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-text-source-resource-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        for name in ["app", "text"] {
            std::fs::create_dir_all(root.join(name).join("src")).unwrap();
        }
        std::fs::write(root.join("app/src/main.ox"), source).unwrap();
        std::fs::write(root.join("app/src/flat.ox"), format!("mod text;\n{source}")).unwrap();
        std::fs::write(root.join("app/src/text.ox"), TEXT_SOURCE).unwrap();
        std::fs::write(root.join("text/src/lib.ox"), TEXT_SOURCE).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn specs(&self) -> [PackageSpec; 2] {
        [
            PackageSpec {
                root: self.0.join("app"),
                entry: self.0.join("app/src/main.ox"),
                dependencies: vec![PackageDependency {
                    alias: "text".into(),
                    package: 1,
                }],
            },
            PackageSpec {
                root: self.0.join("text"),
                entry: self.0.join("text/src/lib.ox"),
                dependencies: vec![],
            },
        ]
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn exercise(body: &str, skip: usize, allocations: u64, peak: u64) {
    let fixture = Fixture::new(&format!("{IMPORTS}{body}"));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let specs = fixture.specs();
        let package = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
        let library = package
            .project
            .sources()
            .files()
            .iter()
            .find(|file| file.path() == specs[1].entry.to_str().unwrap())
            .unwrap();
        assert_eq!(library.text().as_bytes(), TEXT_SOURCE.as_bytes());
        exercise_owner(SourceOwner::packaged(&package), skip, allocations, peak);
    }
    // The same literal library is an ordinary declared module on every host,
    // including the Windows loader. Keep this path live on Linux/macOS too.
    let project = ProjectSources::load_typed(
        fixture.0.join("app/src/flat.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let text_path = fixture.0.join("app/src/text.ox").canonicalize().unwrap();
    let library = project
        .sources()
        .files()
        .iter()
        .find(|file| file.path() == text_path.to_str().unwrap())
        .unwrap();
    assert_eq!(library.text().as_bytes(), TEXT_SOURCE.as_bytes());
    exercise_owner(SourceOwner::project(&project), skip, allocations, peak);
}

fn exercise_owner(owner: SourceOwner<'_>, skip: usize, allocations: u64, peak: u64) {
    let (program, entry) = check_enum_source(owner).unwrap();
    let expected = byte_buffer::Ledger {
        allocations,
        releases: allocations,
        peak_bytes: peak,
        ..Default::default()
    };
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let limits = execute::Limits::default();
        let ((result, remaining), observed) = byte_buffer::testing::capture(
            byte_buffer::testing::Control {
                fault: Some((byte_buffer::Boundary::Allocate, skip)),
                ..Default::default()
            },
            || execute::observe_witness(&program.witness, entry, limits, policy),
        );
        assert_eq!(result, Ok(Scalar::I32(0)), "{policy:?}");
        assert!(observed.fault_applied, "{policy:?}");
        assert_eq!(observed.before_cleanup, expected, "{policy:?}");
        assert_eq!(observed.after_cleanup, expected, "{policy:?}");
        match policy {
            ExecutionPolicy::Bounded => assert!(remaining < limits.fuel),
            ExecutionPolicy::Hosted | ExecutionPolicy::HostedWithHeapLimit(_) => {
                assert_eq!(remaining, HOSTED_FUEL)
            }
        }
    }
}

#[test]
fn owned_text_source_resource_invalid_and_empty_precede_real_copy_allocation_failure() {
    exercise(FROM_UTF8, 0, 0, 0);
}

#[test]
fn owned_text_source_resource_append_failure_preserves_both_owners_and_retry_succeeds() {
    // Two 40-byte owners plus the retry's 48-byte replacement coexist at peak.
    exercise(APPEND_TEXT, 2, 3, 128);
}

#[test]
fn owned_text_source_resource_reserve_failure_preserves_owner_and_retry_succeeds() {
    // The retry's 48-byte replacement coexists with the original 40-byte owner.
    exercise(RESERVE_BYTES, 1, 2, 88);
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[test]
fn owned_text_source_resource_package_policy_refuses_before_loading_or_allocating() {
    let fixture = Fixture::new(&format!("{IMPORTS}{FROM_UTF8}"));
    let specs = fixture.specs();
    let failure = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap_err();
    assert_eq!(failure.diagnostics.len(), 1);
    assert_eq!(
        (failure.diagnostics[0].code, failure.diagnostics[0].stage),
        ("E0006", "package"),
    );
    assert_eq!(
        failure.diagnostics[0].message,
        "package source policy is qualified only on Linux or macOS",
    );
    assert!(failure.sources.files().is_empty());
    assert_eq!(
        failure.usage,
        crate::frontend::project::SourceUsage::default()
    );
    assert_eq!(failure.allocator.attempts, 0);
    assert!(failure.allocator.trace.is_empty());
}
