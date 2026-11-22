//! Windows MSVC x64 CRT descriptors; no OS handle enters a language carrier.
use super::FileOpenMode;
use std::mem::{size_of, MaybeUninit};

// These roles are separate from the unchanged POSIX helper bank. Each File
// delivery pays the complete bank before a path or OS operation is attempted.
#[allow(dead_code)]
struct ControlCarriers {
    open_arguments: (&'static [u8], FileOpenMode),
    wide_path: MaybeUninit<[u16; super::PATH_BYTES]>,
    conversion_arguments: (u32, u32, *const i8, i32, *mut u16, i32),
    conversion: i32,
    path_pointer: *mut u16,
    flags: i32,
    mode: i32,
    open_ffi_arguments: (*const u16, i32, i32),
    descriptor: i32,
    open_return: Result<i32, i32>,
    read_arguments: (i32, &'static mut [u8]),
    write_arguments: (i32, &'static [u8]),
    read_ffi_arguments: (i32, *mut std::ffi::c_void, u32),
    write_ffi_arguments: (i32, *const std::ffi::c_void, u32),
    extent: u32,
    syscall_count: i32,
    byte_return: Result<usize, i32>,
    status_arguments: i32,
    status_return: Result<(), i32>,
    errno_pointer: *mut i32,
    errno_value: i32,
    error_returns: [i32; 2],
}
#[allow(dead_code)]
struct SeekCarriers {
    arguments: (i32, i64),
    ffi_arguments: (i32, i64, i32),
    position: i64,
    result: Result<(), i32>,
}
pub(super) const fn control_bytes() -> usize {
    size_of::<ControlCarriers>()
}
pub(super) const fn seek_control_bytes() -> usize {
    size_of::<SeekCarriers>()
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn MultiByteToWideChar(
        code_page: u32,
        flags: u32,
        bytes: *const i8,
        length: i32,
        wide: *mut u16,
        capacity: i32,
    ) -> i32;
}
unsafe extern "C" {
    fn _wopen(path: *const u16, flags: i32, ...) -> i32;
    fn _read(fd: i32, bytes: *mut std::ffi::c_void, count: u32) -> i32;
    fn _write(fd: i32, bytes: *const std::ffi::c_void, count: u32) -> i32;
    fn _commit(fd: i32) -> i32;
    fn _close(fd: i32) -> i32;
    fn _lseeki64(fd: i32, offset: i64, origin: i32) -> i64;
    fn _errno() -> *mut i32;
}
fn errno() -> i32 {
    // CRT failures report errno, not the unrelated Win32 GetLastError slot.
    // SAFETY: MSVC's CRT returns this thread's non-null live errno slot.
    unsafe { *_errno() }
}
pub(super) fn open(path: &[u8], mode: FileOpenMode) -> Result<i32, i32> {
    if path.is_empty() {
        return Err(2);
    }
    // The caller already checked length/NUL and prepaid that source pass.
    let mut wide = MaybeUninit::<[u16; super::PATH_BYTES]>::uninit();
    // A valid UTF8 string never needs more UTF16 units than input bytes. One
    // conversion validates the entire bounded input without a heap allocation.
    let converted = unsafe {
        MultiByteToWideChar(
            65001,
            8,
            path.as_ptr().cast(),
            path.len() as i32,
            wide.as_mut_ptr().cast(),
            (super::PATH_BYTES - 1) as i32,
        )
    };
    if converted == 0 {
        return Err(22);
    }
    // The conversion initialized precisely this prefix; append its NUL. No
    // uninitialized tail is read or exposed as a Rust reference.
    unsafe {
        wide.as_mut_ptr()
            .cast::<u16>()
            .add(converted as usize)
            .write(0);
    }
    let access = match mode {
        FileOpenMode::Read => 0,
        FileOpenMode::Create => 0x0001 | 0x0100 | 0x0200,
        FileOpenMode::Append => 0x0001 | 0x0100 | 0x0008,
    };
    let flags = 0x8000 | 0x0080 | access;
    // _O_BINARY | _O_NOINHERIT; create uses _S_IREAD | _S_IWRITE.
    let fd = unsafe { _wopen(wide.as_ptr().cast(), flags, 0x0180i32) };
    if fd < 0 {
        Err(errno())
    } else {
        Ok(fd)
    }
}
pub(super) fn read(fd: i32, bytes: &mut [u8]) -> Result<usize, i32> {
    if bytes.len() > i32::MAX as usize {
        return Err(22);
    }
    // SAFETY: The registry owns fd and the caller loans this initialized extent.
    let count = unsafe { _read(fd, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
    if count < 0 {
        Err(errno())
    } else {
        Ok(count as usize)
    }
}
pub(super) fn write(fd: i32, bytes: &[u8]) -> Result<usize, i32> {
    if bytes.len() > i32::MAX as usize {
        return Err(22);
    }
    // SAFETY: The registry owns fd and the caller loans this initialized extent.
    let count = unsafe { _write(fd, bytes.as_ptr().cast(), bytes.len() as u32) };
    if count < 0 {
        Err(errno())
    } else {
        Ok(count as usize)
    }
}
pub(super) fn sync(fd: i32) -> Result<(), i32> {
    // SAFETY: The registry owns this live CRT descriptor.
    if unsafe { _commit(fd) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}
pub(super) fn close(fd: i32) -> Result<(), i32> {
    // One close attempt only. The owner invalidates its descriptor before any
    // later cleanup can observe it; a reused CRT descriptor is never retried.
    // SAFETY: This execution uniquely owns the live registry descriptor.
    if unsafe { _close(fd) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}
pub(super) fn seek(fd: i32, offset: i64) -> Result<(), i32> {
    // SAFETY: The checked registry owns fd; MSVC exposes signed 64-bit offsets.
    let position = unsafe { _lseeki64(fd, offset, 0) };
    if position < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crt_file_and_seek_carriers_have_closed_physical_inventories() {
        // Exhaustive patterns deliberately reject an unreviewed new role.
        fn control(bank: &ControlCarriers) -> usize {
            let ControlCarriers {
                open_arguments,
                wide_path,
                conversion_arguments,
                conversion,
                path_pointer,
                flags,
                mode,
                open_ffi_arguments,
                descriptor,
                open_return,
                read_arguments,
                write_arguments,
                read_ffi_arguments,
                write_ffi_arguments,
                extent,
                syscall_count,
                byte_return,
                status_arguments,
                status_return,
                errno_pointer,
                errno_value,
                error_returns,
            } = bank;
            let fields = [
                std::mem::size_of_val(open_arguments),
                std::mem::size_of_val(wide_path),
                std::mem::size_of_val(conversion_arguments),
                std::mem::size_of_val(conversion),
                std::mem::size_of_val(path_pointer),
                std::mem::size_of_val(flags),
                std::mem::size_of_val(mode),
                std::mem::size_of_val(open_ffi_arguments),
                std::mem::size_of_val(descriptor),
                std::mem::size_of_val(open_return),
                std::mem::size_of_val(read_arguments),
                std::mem::size_of_val(write_arguments),
                std::mem::size_of_val(read_ffi_arguments),
                std::mem::size_of_val(write_ffi_arguments),
                std::mem::size_of_val(extent),
                std::mem::size_of_val(syscall_count),
                std::mem::size_of_val(byte_return),
                std::mem::size_of_val(status_arguments),
                std::mem::size_of_val(status_return),
                std::mem::size_of_val(errno_pointer),
                std::mem::size_of_val(errno_value),
                std::mem::size_of_val(error_returns),
            ];
            fields.into_iter().sum()
        }
        fn seek(bank: &SeekCarriers) -> usize {
            let SeekCarriers {
                arguments,
                ffi_arguments,
                position,
                result,
            } = bank;
            std::mem::size_of_val(arguments)
                + std::mem::size_of_val(ffi_arguments)
                + std::mem::size_of_val(position)
                + std::mem::size_of_val(result)
        }
        let _: fn(&ControlCarriers) -> usize = control;
        let _: fn(&SeekCarriers) -> usize = seek;
        assert_eq!(
            size_of::<[u16; super::super::PATH_BYTES]>(),
            size_of::<MaybeUninit<[u16; super::super::PATH_BYTES]>>()
        );
        println!(
            "Windows CRT File control={} align={} seek={} seek-align={}",
            control_bytes(),
            std::mem::align_of::<ControlCarriers>(),
            seek_control_bytes(),
            std::mem::align_of::<SeekCarriers>()
        );
    }
}
