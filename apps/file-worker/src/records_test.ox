mod jobs;
mod report;

fn main() -> i32 {
    let empty = b"";
    let ordinary = b"data with spaces.bin";
    let embedded_nul = b"data\0suffix";
    if crate::jobs::valid_path(&empty) || !crate::jobs::valid_path(&ordinary)
    || crate::jobs::valid_path(&embedded_nul) { return 1; }

    let mut digits = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    let zero = crate::report::decimal(0u64, &mut digits);
    if zero != 19 || digits[19] != 48u8 { return 2; }
    let max = crate::report::decimal(18446744073709551615u64, &mut digits);
    if max != 0 { return 3; }
    let expected = b"18446744073709551615";
    for index in 0..20 {
        if digits[index] != expected[index] { return 4; }
    }
    return 0;
}
