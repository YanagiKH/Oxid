use std::fs::File;
use std::fs::OpenResult;
use std::fs::create;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::truncate;
use alloc::vec::take;
struct Message { value: i32, file: File }
fn read_and_drop(message: Message) -> i32 { return message.value; }
fn main() -> i32 {
    let mut values = new::<Message>();
    try_reserve(&mut values, 8);
    let reserved = capacity(&values);
    let first_path = b"popped.bin";
    let first = create(&first_path);
    match first {
        OpenResult::Opened(a) => {
            let second_path = b"truncated.bin";
            let second = create(&second_path);
            match second {
                OpenResult::Opened(b) => {
                    push_reserved(&mut values, Message { value: 41, file: a });
                    let length = len(&values);
                    let number = read_and_drop(pop_last(&mut values));
                    push_reserved(&mut values, Message { value: 42, file: b });
                    let mut taken = take(&mut values);
                    truncate(&mut taken, 0);
                    return number + reserved + length + capacity(&taken) + capacity(&values);
                },
                OpenResult::Error(code) => { return -19; },
                OpenResult::MemoryLimit => { return -20; },
                OpenResult::AllocationFailed => { return -21; },
            }
        },
        OpenResult::Error(code) => { return -19; },
        OpenResult::MemoryLimit => { return -20; },
        OpenResult::AllocationFailed => { return -21; },
    }
}
