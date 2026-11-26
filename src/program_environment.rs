//! Read-only access to the environment of a typed process.
use std::mem::size_of;

const NAME_LIMIT: usize = 255;
const VALUE_LIMIT: usize = 32_768;
const INVALID_NAME: i32 = 22;
const OUT_OF_RANGE: i32 = 34;

pub(crate) enum Read {
    Data(i32),
    Missing,
    Error(i32),
}

pub(crate) struct Name {
    bytes: [u8; NAME_LIMIT + 1],
}

impl Name {
    pub(crate) const CAPACITY: usize = NAME_LIMIT + 1;

    /// The name is already copied into fixed storage by the language caller.
    pub(crate) fn from_bytes(input: &[u8], fuel: &mut usize) -> Result<Option<Self>, ()> {
        if input.is_empty() || input.len() > NAME_LIMIT {
            return Ok(None);
        }
        *fuel = fuel.checked_sub(input.len() + 1).ok_or(())?;
        if input
            .iter()
            .any(|byte| matches!(byte, 0 | b'=') || !byte.is_ascii())
        {
            return Ok(None);
        }
        let mut bytes = [0; NAME_LIMIT + 1];
        bytes[..input.len()].copy_from_slice(input);
        Ok(Some(Self { bytes }))
    }
}

pub(crate) fn read(
    name: Option<Name>,
    destination: &mut [u8],
    fuel: &mut usize,
) -> Result<Read, ()> {
    match name {
        Some(name) => read_host(&name, destination, fuel),
        None => Ok(Read::Error(INVALID_NAME)),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_host(name: &Name, destination: &mut [u8], fuel: &mut usize) -> Result<Read, ()> {
    // SAFETY: The validated name is NUL terminated. Oxid only reads these
    // strings in a standalone process and has no environment mutation API.
    let source = unsafe { libc::getenv(name.bytes.as_ptr().cast()) };
    if source.is_null() {
        return Ok(Read::Missing);
    }
    let scan = (*fuel).min(VALUE_LIMIT + 1);
    let length = unsafe { libc::strnlen(source, scan) };
    if length == scan {
        return if scan <= VALUE_LIMIT {
            Err(())
        } else {
            *fuel -= scan;
            Ok(Read::Error(OUT_OF_RANGE))
        };
    }
    let after_scan = *fuel - length - 1;
    if length > destination.len() {
        *fuel = after_scan;
        return Ok(Read::Error(OUT_OF_RANGE));
    }
    *fuel = after_scan.checked_sub(length).ok_or(())?;
    if length != 0 {
        let bytes = unsafe { std::slice::from_raw_parts(source.cast::<u8>(), length) };
        destination[..length].copy_from_slice(bytes);
    }
    Ok(Read::Data(length as i32))
}

#[cfg(all(target_os = "windows", target_env = "msvc", target_arch = "x86_64"))]
fn read_host(name: &Name, destination: &mut [u8], fuel: &mut usize) -> Result<Read, ()> {
    unsafe extern "C" {
        fn _wgetenv(name: *const u16) -> *mut u16;
    }
    let wide = name.bytes.map(u16::from);
    // SAFETY: ASCII input was converted to a NUL-terminated UTF-16 name.
    // The standalone program does not modify the inherited environment.
    let pointer: *const u16 = unsafe { _wgetenv(wide.as_ptr()) };
    if pointer.is_null() {
        return Ok(Read::Missing);
    }
    let mut remaining = *fuel;
    let mut length = 0;
    let mut cursor = pointer;
    while let Some(character) = next_wide_character(&mut cursor) {
        let width = character.len_utf8();
        remaining = remaining.checked_sub(width).ok_or(())?;
        length += width;
        if length > VALUE_LIMIT {
            *fuel = remaining;
            return Ok(Read::Error(OUT_OF_RANGE));
        }
    }
    remaining = remaining.checked_sub(1).ok_or(())?;
    if length > destination.len() {
        *fuel = remaining;
        return Ok(Read::Error(OUT_OF_RANGE));
    }
    *fuel = remaining.checked_sub(length).ok_or(())?;
    let mut offset = 0;
    let mut cursor = pointer;
    while let Some(character) = next_wide_character(&mut cursor) {
        offset += character
            .encode_utf8(&mut destination[offset..length])
            .len();
    }
    Ok(Read::Data(length as i32))
}

#[cfg(all(target_os = "windows", target_env = "msvc", target_arch = "x86_64"))]
fn next_wide_character(cursor: &mut *const u16) -> Option<char> {
    // SAFETY: Each call starts within the NUL-terminated CRT value.
    // A surrogate may inspect the next code unit, including the terminator.
    unsafe {
        let first = **cursor;
        if first == 0 {
            return None;
        }
        *cursor = cursor.add(1);
        let mut codepoint = u32::from(first);
        if (0xd800..=0xdbff).contains(&first) {
            let second = **cursor;
            if (0xdc00..=0xdfff).contains(&second) {
                *cursor = cursor.add(1);
                codepoint =
                    0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
            }
        }
        Some(char::from_u32(codepoint).unwrap_or(char::REPLACEMENT_CHARACTER))
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    all(target_os = "windows", target_env = "msvc", target_arch = "x86_64")
)))]
fn read_host(_: &Name, _: &mut [u8], _: &mut usize) -> Result<Read, ()> {
    Ok(Read::Error(OUT_OF_RANGE))
}

#[allow(dead_code)]
struct ControlCarriers {
    terminated_name: [u8; Name::CAPACITY],
    name: Option<Name>,
    name_result: Result<Option<Name>, ()>,
    destination: &'static mut [u8],
    source: *const u8,
    budget: usize,
    remaining: usize,
    scan_length: usize,
    value_length: usize,
    result: Result<Read, ()>,
    #[cfg(target_os = "windows")]
    wide: [u16; Name::CAPACITY],
    #[cfg(target_os = "windows")]
    wide_cursor: *const u16,
    #[cfg(target_os = "windows")]
    wide_unit: [u16; 2],
    #[cfg(target_os = "windows")]
    codepoint: u32,
    #[cfg(target_os = "windows")]
    character: Option<char>,
    #[cfg(target_os = "windows")]
    character_width: usize,
    #[cfg(target_os = "windows")]
    copy_offset: usize,
}

pub(crate) const fn control_bytes() -> usize {
    size_of::<ControlCarriers>()
}
