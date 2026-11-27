#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = r#"
use alloc::vec::new;
use alloc::vec::try_push;
use alloc::vec::try_resize;
use alloc::vec::capacity;
use alloc::vec::len;
use alloc::vec::truncate;
use alloc::vec::GrowStatus;
use crate::owned_vec::PopResult;
use crate::owned_vec::try_append_copy;
use crate::owned_vec::pop_copy;

fn main() -> i32 {
    let mut source = new::<i32>();
    let mut destination = new::<i32>();
    try_push(&mut source, 40);
    try_resize(&mut destination, 8, 2);
    let source_capacity = capacity(&source);
    let destination_capacity = capacity(&destination);
    let status = try_append_copy(&mut destination, &mut source);
    match status {
        GrowStatus::Ok => { return -1; },
        GrowStatus::CapacityOverflow => { return -2; },
        GrowStatus::AllocationFailed => { return -3; },
        GrowStatus::MemoryLimit => {},
    }
    if len(&source) != 1 || source[0] != 40 || capacity(&source) != source_capacity
        || len(&destination) != 8 || destination[7] != 2
        || capacity(&destination) != destination_capacity { return -4; }

    truncate(&mut destination, 7);
    let retried = try_append_copy(&mut destination, &mut source);
    match retried {
        GrowStatus::Ok => {},
        GrowStatus::CapacityOverflow => { return -5; },
        GrowStatus::AllocationFailed => { return -6; },
        GrowStatus::MemoryLimit => { return -7; },
    }
    if len(&source) != 0 || capacity(&source) != source_capacity { return -8; }
    let removed = pop_copy(&mut destination);
    match removed {
        PopResult::Empty => { return -9; },
        PopResult::Value(value) => { return value + destination[6]; },
    }
}
"#;

struct Project(PathBuf);

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exhausted_copy_append_preserves_both_vectors_for_retry() {
    let root = std::env::temp_dir().join(format!("oxid-copy-append-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let project = Project(root);
    let root = &project.0;
    fs::create_dir(root.join("src")).unwrap();
    fs::create_dir_all(root.join("owned_vec/src")).unwrap();
    fs::write(
        root.join("owned_vec/oxid.toml"),
        include_bytes!("../packages/owned_vec/oxid.toml"),
    )
    .unwrap();
    fs::write(
        root.join("owned_vec/src/lib.ox"),
        include_bytes!("../packages/owned_vec/src/lib.ox"),
    )
    .unwrap();
    // Two eight-element i32 allocations fit. Reallocating the destination
    // while both are live exceeds the limit.
    fs::write(
        root.join("oxid.toml"),
        "[project]\nname=\"copy-append-test\"\nedition=\"typed-preview\"\n\
         [build]\nentry-mode=\"result\"\n\
         [dependencies]\nowned_vec=\"owned_vec\"\n\
         [execution]\npolicy=\"hosted\"\nheap-limit=160\n",
    )
    .unwrap();
    fs::write(root.join("src/main.ox"), SOURCE).unwrap();

    let oxid = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .env_remove("OXID_PATH")
            .current_dir(root)
            .output()
            .unwrap()
    };
    let locked = oxid(&["lock", "--offline"]);
    assert!(locked.status.success(), "{locked:?}");
    let ran = oxid(&["run", "--frozen"]);
    assert!(ran.status.success() && ran.stderr.is_empty(), "{ran:?}");
    assert_eq!(ran.stdout, b"42\n");
}
