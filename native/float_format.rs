#![no_std]
use core::fmt::{self, Write};
struct Buffer<'a> {
    output: &'a mut [u8],
    length: usize,
}
impl Write for Buffer<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let Some(end) = self.length.checked_add(text.len()) else {
            return Err(fmt::Error);
        };
        let Some(destination) = self.output.get_mut(self.length..end) else {
            return Err(fmt::Error);
        };
        destination.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
#[no_mangle]
pub unsafe extern "C" fn oxid_format_float(
    bits: u64,
    width: u32,
    output: *mut u8,
    capacity: usize,
) -> usize {
    let output = unsafe { core::slice::from_raw_parts_mut(output, capacity) };
    let mut buffer = Buffer { output, length: 0 };
    let result = match width {
        32 => write!(&mut buffer, "{}", f32::from_bits(bits as u32)),
        64 => write!(&mut buffer, "{}", f64::from_bits(bits)),
        _ => return 0,
    };
    if result.is_err() {
        0
    } else {
        buffer.length
    }
}
unsafe extern "C" {
    fn abort() -> !;
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe { abort() }
}
// The distributed core archive carries this DWARF reference even with panic=abort.
// The helper never unwinds across C; an unexpected personality call terminates.
#[no_mangle]
pub extern "C" fn rust_eh_personality() {
    unsafe { abort() }
}
