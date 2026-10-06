//! Closed, unbuffered stdin adapter. One call is exactly one one-byte attempt.
//! Retry policy and fuel belong to the verified reference consumer.

pub(super) enum Attempt {
    Byte(u8),
    Eof,
    Interrupted,
    Error,
}

pub(super) const fn supported_host() -> bool {
    cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(super) fn read_one() -> Attempt {
    unsafe extern "C" {
        fn read(fd: std::ffi::c_int, buffer: *mut std::ffi::c_void, count: usize) -> isize;
    }
    let mut byte = 0u8;
    // SAFETY: This is the Linux x86_64 read ABI. fd 0 is inherited stdin;
    // the live, writable byte is valid for the exact count 1. The unbuffered
    // call makes one attempt; this adapter never retries or changes signals.
    let result = unsafe { read(0, (&mut byte as *mut u8).cast(), 1) };
    match result {
        1 => Attempt::Byte(byte),
        0 => Attempt::Eof,
        result
            if result < 0
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted =>
        {
            Attempt::Interrupted
        }
        _ => Attempt::Error,
    }
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
)))]
pub(super) fn read_one() -> Attempt {
    // Input-bearing execution is rejected before activation on these hosts.
    // Keep the adapter effect-free even if an internal caller violates that gate.
    Attempt::Error
}
