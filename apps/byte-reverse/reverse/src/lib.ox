pub fn reverse(bytes: &mut [u8]) -> () {
    let mut left = 0;
    let mut right = bytes.len();
    while left < right {
        right = right - 1;
        let saved = bytes[left];
        bytes[left] = bytes[right];
        bytes[right] = saved;
        left = left + 1;
    }
    return;
}
