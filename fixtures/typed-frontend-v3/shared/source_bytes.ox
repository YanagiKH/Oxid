// Packed ASCII source; caller proves 0 <= offset < used <= 511.
// put additionally requires byte in 0..127 and fresh destination lane.
pub fn zeros() -> [i32; 128] { return [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; }
pub fn factor(offset: i32) -> i32 {
    let lane = offset % 4;
    if lane == 0 { return 1; }
    if lane == 1 { return 128; }
    if lane == 2 { return 16384; }
    return 2097152;
}
pub fn at(codes: &[i32], offset: i32) -> i32 {
    let value = codes[offset / 4];
    let lane = offset % 4;
    if lane == 0 { return value % 128; }
    if lane == 1 { return value / 128 % 128; }
    if lane == 2 { return value / 16384 % 128; }
    return value / 2097152 % 128;
}
pub fn put(codes: &mut [i32], offset: i32, byte: i32) -> () { codes[offset / 4] = codes[offset / 4] + byte * factor(offset); return; }
