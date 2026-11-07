use super::*;

fn exercise(text: &str, expected: i32, allocations: u64, peak: u64, files: &[PathBuf]) {
    with_typed(text, |typed, sources, fixture| {
        let associated =
            super::super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) = super::super::super::super::byte_buffer::testing::capture(
                Default::default(),
                || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                },
            );
            assert_eq!(result, Ok(Scalar::I32(expected)));
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup.allocations, allocations);
            assert_eq!(observed.before_cleanup.releases, allocations);
            assert_eq!(observed.before_cleanup.peak_bytes, peak);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
            #[cfg(target_os = "linux")]
            for descriptor in std::fs::read_dir("/proc/self/fd").unwrap() {
                if let Ok(target) = std::fs::read_link(descriptor.unwrap().path()) {
                    assert!(
                        !files.contains(&target),
                        "retained File descriptor: {}",
                        target.display()
                    );
                }
            }
        }
        if std::env::var_os("OXID_TEST_LLVM_BIN").is_some() {
            assert_native_outputs(typed, sources, fixture, expected);
        }
    });
}

#[test]
fn owned_vector_source_execution_active_resource_enums_keep_alignment_and_release() {
    let imports = IMPORTS.replace(
        "struct Message{value:i32,bytes:ByteBuf}",
        "enum Payload{Empty,Bytes(ByteBuf),Wide(i128)}struct Message{value:i32,payload:Payload}",
    );
    let body = BODY.replace(
        "let mut bytes=bytes_new();bytes_push(&mut bytes,value.to_u8_checked());return Message{value:value,bytes:bytes};",
        "if value==42{return Message{value:value,payload:Payload::Empty};}let mut bytes=bytes_new();bytes_push(&mut bytes,value.to_u8_checked());return Message{value:value,payload:Payload::Bytes(bytes)};",
    );
    assert_ne!(imports, IMPORTS);
    assert_ne!(body, BODY);
    // A 48-byte Message at capacity8 plus its64-byte header and64-byte
    // identity area occupies512; one live40-byte buffer makes552.
    exercise(&format!("{imports}{body}"), 58, 2, 552, &[]);
}

#[test]
fn owned_vector_source_execution_active_wide_enum_destructuring_and_match_release() {
    exercise(
        include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox"),
        100,
        3,
        552,
        &[],
    );
}

#[test]
fn owned_vector_source_execution_resource_enum_replacement_commits_after_admission() {
    let source = include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox");
    let (declarations, _) = source.split_once("fn main() -> i32 {").unwrap();
    let main = "fn main()->i32{let mut values=new::<Message>();try_reserve(&mut values,1);let mut message=make(42);message=make(41);message=make(42);push_reserved(&mut values,message);return read_and_drop(pop_last(&mut values));}";
    exercise(&format!("{declarations}{main}"), 42, 3, 552, &[]);
}

fn file_source(first: &std::path::Path, second: &std::path::Path) -> String {
    let imports = "use std::fs::File;use std::fs::OpenResult;use std::fs::create;use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::truncate;use alloc::vec::take;struct Message{value:i32,file:File}fn read_and_drop(message:Message)->i32{return message.value;}";
    let first = first.to_str().unwrap();
    let second = second.to_str().unwrap();
    format!("{imports}fn main()->i32{{let mut values=new::<Message>();try_reserve(&mut values,8);let reserved=capacity(&values);let first_path=b{first:?};let first=create(&first_path);match first{{OpenResult::Opened(a)=>{{let second_path=b{second:?};let second=create(&second_path);match second{{OpenResult::Opened(b)=>{{push_reserved(&mut values,Message{{value:41,file:a}});let length=len(&values);let number=read_and_drop(pop_last(&mut values));push_reserved(&mut values,Message{{value:42,file:b}});let mut taken=take(&mut values);truncate(&mut taken,0);return number+reserved+length+capacity(&taken)+capacity(&values);}},OpenResult::Error(code)=>{{return -19;}},OpenResult::MemoryLimit=>{{return -20;}},OpenResult::AllocationFailed=>{{return -21;}},}}}},OpenResult::Error(code)=>{{return -19;}},OpenResult::MemoryLimit=>{{return -20;}},OpenResult::AllocationFailed=>{{return -21;}},}}}}")
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owned_vector_source_execution_files_close_across_match_pop_take_and_truncate() {
    let scope = Fixture::new("");
    let directory = std::fs::canonicalize(&scope.0).unwrap();
    let files = [directory.join("popped"), directory.join("truncated")];
    // The two actual File handles coexist with the320-byte vector allocation.
    exercise(&file_source(&files[0], &files[1]), 58, 3, 384, &files);
    for file in files {
        assert!(file.is_file());
        std::fs::write(file, b"reopened after source execution").unwrap();
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owned_vector_source_execution_file_error_releases_the_existing_vector() {
    let scope = Fixture::new("");
    let first = scope.0.join("missing").join("file");
    let second = scope.0.join("unused");
    exercise(&file_source(&first, &second), -19, 2, 352, &[]);
    assert!(!first.exists() && !second.exists());
}
