//! Closed, unbuffered stdout adapter. One call is exactly one one-byte attempt.
//! Process entry owns SIGPIPE policy; this adapter never installs or changes it.
//! Retry policy and fuel belong to the verified reference consumer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attempt {
    Accepted,
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

fn classify(result: isize, interrupted: bool) -> Attempt {
    match result {
        1 => Attempt::Accepted,
        result if result < 0 && interrupted => Attempt::Interrupted,
        _ => Attempt::Error,
    }
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(super) fn write_one(byte: u8) -> Attempt {
    unsafe extern "C" {
        fn write(fd: std::ffi::c_int, buffer: *const std::ffi::c_void, count: usize) -> isize;
    }
    // SAFETY: This is the Linux x86_64 write ABI. fd 1 is inherited stdout;
    // the live, initialized byte is readable for the exact count 1. The libc
    // write boundary makes one unbuffered attempt and does not retry EINTR.
    // Read its error immediately, before any other host operation can alter it.
    let result = unsafe { write(1, (&byte as *const u8).cast(), 1) };
    let interrupted =
        result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted;
    classify(result, interrupted)
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
)))]
pub(super) fn write_one(_byte: u8) -> Attempt {
    // Output-bearing execution is rejected before activation on these hosts.
    // A violated internal gate still cannot perform an output effect here.
    Attempt::Error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_byte_syscall_results_have_no_implicit_progress_or_retry() {
        assert_eq!(classify(1, false), Attempt::Accepted);
        assert_eq!(classify(-1, true), Attempt::Interrupted);
        // Zero progress, EAGAIN, EPIPE and all other errors stop this call.
        // A claimed short count greater than the request also fails closed.
        for result in [0, -1, isize::MIN, 2, isize::MAX] {
            assert_eq!(classify(result, false), Attempt::Error, "result={result}");
        }
        // A stale EINTR cannot change a successful or zero-progress result.
        for (result, expected) in [
            (1, Attempt::Accepted),
            (0, Attempt::Error),
            (2, Attempt::Error),
        ] {
            assert_eq!(classify(result, true), expected, "result={result}");
        }
    }
}
