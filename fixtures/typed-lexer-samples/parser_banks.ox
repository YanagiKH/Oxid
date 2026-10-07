// Admission-only lossless carriers. No parser grammar is implemented here.
pub struct Rows {
    pub header_links: [i32; 129], pub ab: [i32; 129], pub cd: [i32; 129], pub count: i32,
}
pub struct Frames { pub slot: [i32; 129], pub count: i32 }

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
pub fn seed(stack: &mut Frames) -> bool {
    if stack.count != 0 { return false; }
    stack.slot[0] = 1;
    stack.count = 1;
    return true;
}
pub fn push(stack: &mut Frames, state: i32, node: i32, aux: i32) -> bool {
    if stack.count < 1 || stack.count >= 129 { return false; }
    if node < 1 || node > 128 || state < 2 { return false; }
    let control = pack_control(state, aux);
    if control < 0 { return false; }
    let mut i = 0;
    while i < stack.count {
        if stack.slot[i] / 65536 == node { return false; }
        i = i + 1;
    }
    stack.slot[stack.count] = node * 65536 + control;
    stack.count = stack.count + 1;
    return true;
}
pub fn resume(stack: &mut Frames, state: i32, aux: i32) -> bool {
    if stack.count < 2 || stack.count > 129 || state < 2 { return false; }
    let control = pack_control(state, aux);
    if control < 0 { return false; }
    let node = stack.slot[stack.count - 1] / 65536;
    if node < 1 || node > 128 { return false; }
    stack.slot[stack.count - 1] = node * 65536 + control;
    return true;
}
pub fn pop(stack: &mut Frames) -> i32 {
    if stack.count < 2 || stack.count > 129 { return -1; }
    let node = stack.slot[stack.count - 1] / 65536;
    if node < 1 || node > 128 { return -1; }
    stack.count = stack.count - 1;
    stack.slot[stack.count] = 0;
    return node;
}
pub fn fill(rows: &mut Rows) -> () {
    let mut i = 0;
    while i < 128 {
        let mut next = i + 2;
        if i == 127 { next = 0; }
        rows.header_links[i] = pack_header(2 + i % 20, i, i + 1, next);
        rows.ab[i] = pack_pair(i + 1, 128 - i);
        rows.cd[i] = pack_pair(i % 2, 1 + i % 64);
        i = i + 1;
    }
    rows.count = 128;
    return;
}
pub fn valid(rows: &Rows) -> bool {
    if rows.count != 128 { return false; }
    let mut i = 0;
    while i < 128 {
        let header = rows.header_links[i];
        if header < 0 || header > 538976319 { return false; }
        if header % 64 != 2 + i % 20 || header / 64 % 65536 != i + 256 * (i + 1) { return false; }
        if rows.ab[i] % 256 != i + 1 || rows.ab[i] / 256 != 128 - i { return false; }
        if rows.cd[i] % 256 != i % 2 || rows.cd[i] / 256 != 1 + i % 64 { return false; }
        if i < 127 { if header / 4194304 != i + 2 { return false; } }
        else { if header / 4194304 != 0 { return false; } }
        i = i + 1;
    }
    return rows.header_links[128] == 0 && rows.ab[128] == 0 && rows.cd[128] == 0;
}
