//! Standalone rustc client; compiled by the explicit real-toolchain integration test.
use std::sync::{Arc, Barrier};

#[link(name = "oxid_library")]
unsafe extern "C" {
    fn oxid_library_abi_version() -> u32;
    fn oxid_library_call_v1(unit: i32, quantity: i32, divisor: i32, out: *mut i32) -> i32;
}

fn line_total(unit: i32, quantity: i32, divisor: i32) -> Result<i32, i32> {
    let mut output = 123456789;
    let status = unsafe { oxid_library_call_v1(unit, quantity, divisor, &mut output) };
    if status == 0 {
        Ok(output)
    } else {
        assert_eq!(output, 123456789, "error changed caller output");
        Err(status)
    }
}

fn examples(fuel_only: bool) {
    for (unit, quantity, divisor, expected) in [
        (1250, 3, 2, Ok(1875)),
        (-7, 1, 2, Ok(-3)),
        (0, 1, 1, Ok(0)),
        (i32::MIN, 1, 1, Ok(i32::MIN)),
        (i32::MAX, 1, 1, Ok(i32::MAX)),
        (i32::MAX, 2, 1, Err(1)),
        (i32::MIN, 1, -1, Err(1)),
        (1, 1, 0, Err(2)),
        (42, 1, 1, Ok(42)),
    ] {
        assert_eq!(
            line_total(unit, quantity, divisor),
            if fuel_only { Err(3) } else { expected }
        );
    }
    assert_eq!(
        unsafe { oxid_library_call_v1(1, 1, 1, std::ptr::null_mut()) },
        4
    );
    let mut bytes = [0xa5_u8; 16];
    let base = bytes.as_mut_ptr();
    let offset = (0..4)
        .find(|offset| (base as usize + offset) % 4 != 0)
        .unwrap();
    let pointer = unsafe { base.add(offset).cast::<i32>() };
    assert_eq!(unsafe { oxid_library_call_v1(1, 1, 1, pointer) }, 4);
    assert_eq!(
        bytes, [0xa5; 16],
        "invalid output argument changed backing storage"
    );
}

fn main() {
    let mode = std::env::args().nth(1).expect("pricing|fuel mode");
    assert!(mode == "pricing" || mode == "fuel");
    let fuel_only = mode == "fuel";
    assert_eq!(unsafe { oxid_library_abi_version() }, 1);
    examples(fuel_only);
    let barrier = Arc::new(Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..128 {
                    examples(fuel_only);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    examples(fuel_only);
    println!("native Rust calls passed");
}
