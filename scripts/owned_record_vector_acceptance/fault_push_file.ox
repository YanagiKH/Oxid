use std::fs::File;
use std::fs::OpenResult;
use std::fs::create;
use alloc::vec::new;
use alloc::vec::push_reserved;
struct Message { file: File }
fn main() -> i32 {
    let mut values = new::<Message>();
    let path = b"failure-file.bin";
    let opened = create(&path);
    match opened {
        OpenResult::Opened(file) => {
            push_reserved(&mut values, Message { file: file });
            return 99;
        },
        OpenResult::Error(code) => { return -19; },
        OpenResult::MemoryLimit => { return -20; },
        OpenResult::AllocationFailed => { return -21; },
    }
}
