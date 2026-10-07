// Admission-only separate column owners; logical packing is unchanged.
pub struct Counts { pub rows: i32, pub stack: i32 }

pub fn pack_header(kind: i32, start: i32, end: i32, next: i32) -> i32 {
    if kind < 1 || kind > 63 || start < 0 || start > end || end > 128 || next < 0 || next > 128 { return -1; }
    return kind + 64 * (start + 256 * end) + 4194304 * next;
}
pub fn pack_pair(a: i32, b: i32) -> i32 {
    if a < 0 || a > 128 || b < 0 || b > 128 { return -1; }
    return a + 256 * b;
}
fn pack_control(state: i32, aux: i32) -> i32 {
    if state < 1 || state > 21 || aux < 0 || aux > 1984 || aux % 128 > 64 { return -1; }
    return state + 32 * aux;
}
fn frame_node(slot: i32) -> i32 {
    if slot < 65536 || slot > 8452117 { return -1; }
    let node = slot / 65536;
    let control = slot % 65536;
    let state = control % 32;
    let aux = control / 32;
    if node > 128 || state < 2 || state > 21 || aux > 1984 || aux % 128 > 64 { return -1; }
    return node;
}
pub fn seed(stack: &mut [i32], counts: &mut Counts) -> bool {
    if stack.len() != 129 || counts.stack != 0 { return false; }
    stack[0] = 1;
    counts.stack = 1;
    return true;
}
pub fn push(stack: &mut [i32], counts: &mut Counts, state: i32, node: i32, aux: i32) -> bool {
    if stack.len() != 129 || counts.stack < 1 || counts.stack >= 129 { return false; }
    if node < 1 || node > 128 || state < 2 { return false; }
    let control = pack_control(state, aux);
    if control < 0 { return false; }
    let mut i = 0;
    while i < counts.stack {
        if stack[i] / 65536 == node { return false; }
        i = i + 1;
    }
    stack[counts.stack] = node * 65536 + control;
    counts.stack = counts.stack + 1;
    return true;
}
pub fn resume(stack: &mut [i32], counts: &mut Counts, state: i32, aux: i32) -> bool {
    if stack.len() != 129 || counts.stack < 2 || counts.stack > 129 || state < 2 { return false; }
    let control = pack_control(state, aux);
    if control < 0 { return false; }
    let node = frame_node(stack[counts.stack - 1]);
    if node < 1 || node > 128 { return false; }
    stack[counts.stack - 1] = node * 65536 + control;
    return true;
}
pub fn pop(stack: &mut [i32], counts: &mut Counts) -> i32 {
    if stack.len() != 129 || counts.stack < 2 || counts.stack > 129 { return -1; }
    let node = frame_node(stack[counts.stack - 1]);
    if node < 1 || node > 128 { return -1; }
    counts.stack = counts.stack - 1;
    stack[counts.stack] = 0;
    return node;
}
pub fn fill(header: &mut [i32], ab: &mut [i32], cd: &mut [i32], counts: &mut Counts) -> () {
    let mut i = 0;
    while i < 128 {
        let mut next = i + 2;
        if i == 127 { next = 0; }
        header[i] = pack_header(2 + i % 20, i, i + 1, next);
        ab[i] = pack_pair(i + 1, 128 - i);
        cd[i] = pack_pair(i % 2, 1 + i % 64);
        i = i + 1;
    }
    counts.rows = 128;
    return;
}
pub fn valid(headers: &[i32], ab: &[i32], cd: &[i32], count: i32) -> bool {
    if count != 128 || headers.len() != 129 || ab.len() != 129 || cd.len() != 129 { return false; }
    let mut i = 0;
    while i < 128 {
        let header = headers[i];
        if header < 0 || header > 538976319 { return false; }
        if header % 64 != 2 + i % 20 || header / 64 % 65536 != i + 256 * (i + 1) { return false; }
        if ab[i] % 256 != i + 1 || ab[i] / 256 != 128 - i { return false; }
        if cd[i] % 256 != i % 2 || cd[i] / 256 != 1 + i % 64 { return false; }
        if i < 127 { if header / 4194304 != i + 2 { return false; } }
        else { if header / 4194304 != 0 { return false; } }
        i = i + 1;
    }
    return headers[128] == 0 && ab[128] == 0 && cd[128] == 0;
}
