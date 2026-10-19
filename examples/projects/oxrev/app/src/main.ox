use binary::Loaded;
use binary::WrittenAll;
use binary::read_all;
use binary::write_all;
use reverse::reverse;

fn main() -> i32 {
    let loaded = read_all();
    match loaded {
        Loaded::Ok(document) => {
            let mut doc = document;
            reverse(&mut doc.bytes);
            let written = write_all(&doc.bytes);
            match written {
                WrittenAll::Ok => { return 0; },
                WrittenAll::Error(code) => { return 74; },
                WrittenAll::WriteZero => { return 74; },
            }
        },
        Loaded::CapacityOverflow => { return 70; },
        Loaded::AllocationFailed => { return 71; },
        Loaded::MemoryLimit => { return 71; },
        Loaded::IoError(code) => { return 74; },
    }
}
