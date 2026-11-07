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

pub fn put32(bytes: &mut [u8], start: i32, number: u32) -> () {
    for offset in 0..4 { let part = (number >> (offset * 8)) & 255u32; bytes[start + offset] = part.to_u8_checked(); }
    return;
}

pub fn get32(bytes: &[u8], start: i32) -> u32 {
    let mut number = 0u32;
    for offset in 0..4 { let part = bytes[start + offset]; number = number | (part.to_u32_checked() << (offset * 8)); }
    return number;
}

pub fn put64(bytes: &mut [u8], start: i32, number: u64) -> () {
    for offset in 0..8 { let part = (number >> (offset * 8)) & 255u64; bytes[start + offset] = part.to_u8_checked(); }
    return;
}

pub fn get64(bytes: &[u8], start: i32) -> u64 {
    let mut number = 0u64;
    for offset in 0..8 { let part = bytes[start + offset]; number = number | (part.to_u64_checked() << (offset * 8)); }
    return number;
}

pub fn checksum(bytes: &[u8], length: i32) -> u32 {
    let mut crc = 4294967295u32;
    for index in 0..length {
        let part = bytes[index];
        crc = crc ^ part.to_u32_checked();
        for bit in 0..8 {
            if (crc & 1u32) == 1u32 { crc = (crc >> 1) ^ 3988292384u32; }
            else { crc = crc >> 1; }
        }
    }
    return crc ^ 4294967295u32;
}
