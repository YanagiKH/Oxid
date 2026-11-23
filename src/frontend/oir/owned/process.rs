//! Process terminal capability, separate from qualified source execution.
//! Setup precedes every process diagnostic and source effect. A failed setup
//! must be converted directly to silent status 74 by the caller.

#[cfg(any(
    all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
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
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub(in crate::frontend) fn setup() -> bool {
    crate::frontend::build_config::NativeBuildSettings::windows_msvc_host()
}

/// Terminal text after successful setup. Diagnostic retries do not execute
/// source work or consume source fuel. Never recursively report a write error.
#[cfg(any(
    all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
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
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
)))]
pub(in crate::frontend) fn diagnostic(_message: &[u8]) -> i32 {
    74
}

#[cfg(all(
    target_os = "windows",
    target_env = "msvc",
    target_arch = "x86_64",
    target_pointer_width = "64",
    target_endian = "little"
))]
pub(in crate::frontend) fn diagnostic(mut message: &[u8]) -> i32 {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(stream: u32) -> *mut c_void;
        fn WriteFile(
            handle: *mut c_void,
            data: *const c_void,
            length: u32,
            written: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }
    if message.is_empty() {
        return 1;
    }
    // SAFETY: STD_ERROR_HANDLE selects the inherited handle without changing it.
    let handle = unsafe { GetStdHandle(-12_i32 as u32) };
    if handle.is_null() || handle.addr() == usize::MAX {
        return 74;
    }
    while !message.is_empty() {
        let length = message.len().min(u32::MAX as usize) as u32;
        let mut written = 0;
        // SAFETY: The slice covers exactly the admitted readable extent. This
        // is a synchronous write and the count is live until the call returns.
        let ok = unsafe {
            WriteFile(
                handle,
                message.as_ptr().cast(),
                length,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || written == 0 || written > length {
            return 74;
        }
        message = &message[written as usize..];
    }
    1
}

/// POSIX facilities for signals and the existing process I/O providers.
pub(in crate::frontend) fn supported_host() -> bool {
    cfg!(all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))
}

pub(in crate::frontend) fn entry_supported_host() -> bool {
    supported_host() || crate::frontend::build_config::NativeBuildSettings::windows_msvc_host()
}
