pub fn load(bytes: &[u8], element: i32) -> f64 {
    let start = element * 8;
    let mut bits = 0u64;
    for offset in 0..8 {
        let byte = bytes[start + offset];
        bits = bits | (byte.to_u64_checked() << (offset * 8));
    }
    return f64::from_bits(bits);
}

pub fn store(bytes: &mut [u8], element: i32, value: f64) -> () {
    let start = element * 8;
    let bits = value.to_bits();
    for offset in 0..8 {
        let byte = (bits >> (offset * 8)) & 255u64;
        bytes[start + offset] = byte.to_u8_checked();
    }
    return;
}
