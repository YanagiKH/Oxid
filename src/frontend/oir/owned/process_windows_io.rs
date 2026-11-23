//! One unbuffered attempt on an inherited Windows standard handle.
use std::ffi::c_void;

type Handle = *mut c_void;
const STD_INPUT: u32 = -10_i32 as u32;
const STD_OUTPUT: u32 = -11_i32 as u32;
const STD_ERROR: u32 = -12_i32 as u32;
const INVALID_HANDLE: i32 = 6;
const GENERAL_FAILURE: i32 = 31;
const END_OF_FILE: i32 = 38;
const INVALID_PARAMETER: i32 = 87;
const BROKEN_PIPE: i32 = 109;
const PIPE_CLOSING: i32 = 232;
const MORE_DATA: i32 = 234;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(stream: u32) -> Handle;
    fn GetLastError() -> u32;
    fn ReadFile(
        handle: Handle,
        data: *mut c_void,
        length: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        handle: Handle,
        data: *const c_void,
        length: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
}

#[allow(dead_code)]
struct ControlCarriers {
    read: &'static mut [u8],
    write: (&'static [u8], bool),
    stream: u32,
    handle: Handle,
    selected_handle: Result<Handle, i32>,
    read_args: (Handle, *mut c_void, u32, *mut u32, *mut c_void),
    write_args: (Handle, *const c_void, u32, *mut u32, *mut c_void),
    extent: u32,
    transferred: u32,
    status: i32,
    error: i32,
    result: Result<usize, i32>,
}

pub(in crate::frontend::oir::owned) const fn control_bytes() -> usize {
    std::mem::size_of::<ControlCarriers>()
}

fn inherited_handle(stream: u32) -> Result<Handle, i32> {
    // SAFETY: The selector names a standard handle. The caller never closes it.
    let handle = unsafe { GetStdHandle(stream) };
    if handle.is_null() || handle.addr() == usize::MAX {
        Err(INVALID_HANDLE)
    } else {
        Ok(handle)
    }
}

fn last_error() -> i32 {
    // SAFETY: Read the current thread's Win32 status immediately after failure.
    match unsafe { GetLastError() } as i32 {
        0 => GENERAL_FAILURE,
        code => code,
    }
}

pub(in crate::frontend::oir::owned) fn read(bytes: &mut [u8]) -> Result<usize, i32> {
    let length = u32::try_from(bytes.len()).map_err(|_| INVALID_PARAMETER)?;
    let handle = inherited_handle(STD_INPUT)?;
    let mut count = 0;
    // SAFETY: The mutable slice owns the precise writable extent. No buffer
    // or overlapped state survives the synchronous call.
    let ok = unsafe {
        ReadFile(
            handle,
            bytes.as_mut_ptr().cast(),
            length,
            &mut count,
            std::ptr::null_mut(),
        )
    };
    let error = if ok == 0 { last_error() } else { 0 };
    if count > length {
        return Err(GENERAL_FAILURE);
    }
    match error {
        0 => Ok(count as usize),
        MORE_DATA if count != 0 => Ok(count as usize),
        END_OF_FILE | BROKEN_PIPE | PIPE_CLOSING => Ok(0),
        error => Err(error),
    }
}

pub(in crate::frontend::oir::owned) fn write(bytes: &[u8], stderr: bool) -> Result<usize, i32> {
    let length = u32::try_from(bytes.len()).map_err(|_| INVALID_PARAMETER)?;
    let handle = inherited_handle(if stderr { STD_ERROR } else { STD_OUTPUT })?;
    let mut count = 0;
    // SAFETY: The immutable slice is readable for exactly length bytes, and
    // the count is live until the synchronous call returns.
    let ok = unsafe {
        WriteFile(
            handle,
            bytes.as_ptr().cast(),
            length,
            &mut count,
            std::ptr::null_mut(),
        )
    };
    let error = if ok == 0 { last_error() } else { 0 };
    if count > length {
        Err(GENERAL_FAILURE)
    } else if error == 0 || count != 0 {
        Ok(count as usize)
    } else {
        Err(error)
    }
}
