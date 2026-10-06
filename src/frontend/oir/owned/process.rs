//! Process terminal capability, separate from qualified source execution.
//! Setup precedes every process diagnostic and source effect. A failed setup
//! must be converted directly to silent status 74 by the caller.

#[cfg(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
pub(in crate::frontend) fn setup() -> bool {
    unsafe extern "C" {
        fn signal(number: std::ffi::c_int, handler: usize) -> usize;
    }
    // SAFETY: On these explicit Linux/macOS ABIs, SIGPIPE is 13, SIG_IGN is
    // the sentinel 1 and SIG_ERR is all ones. No Rust handler is installed.
    unsafe { signal(13, 1) != usize::MAX }
}

#[cfg(not(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub(in crate::frontend) fn setup() -> bool {
    false
}

/// Terminal text after successful setup. Diagnostic retries do not execute
/// source work or consume source fuel. Never recursively report a write error.
#[cfg(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
pub(in crate::frontend) fn diagnostic(message: &[u8]) -> i32 {
    unsafe extern "C" {
        fn write(fd: std::ffi::c_int, bytes: *const std::ffi::c_void, length: usize) -> isize;
    }
    let mut remaining = message;
    while !remaining.is_empty() {
        // SAFETY: The immutable live slice covers precisely this write extent.
        let done = unsafe { write(2, remaining.as_ptr().cast(), remaining.len()) };
        if done < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if done <= 0 || done as usize > remaining.len() {
            return 74;
        }
        remaining = &remaining[done as usize..];
    }
    1
}

#[cfg(not(any(
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub(in crate::frontend) fn diagnostic(_message: &[u8]) -> i32 {
    74
}

/// Runtime qualification does not follow the wider diagnostic capability.
pub(in crate::frontend) fn supported_host() -> bool {
    cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))
}
