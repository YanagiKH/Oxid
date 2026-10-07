// Admission-only carriers. No parser grammar is implemented here.
pub struct Rows {
    pub kind: [i32; 129], pub span: [i32; 129],
    pub a: [i32; 129], pub b: [i32; 129], pub c: [i32; 129],
    pub d: [i32; 129], pub next: [i32; 129], pub count: i32,
}
pub struct Frames {
    pub state: [i32; 129], pub node: [i32; 129],
    pub aux: [i32; 129], pub count: i32,
}

pub fn seed(stack: &mut Frames) -> bool {
    if stack.count != 0 { return false; }
    stack.state[0] = 1;
    stack.node[0] = 0;
    stack.aux[0] = 0;
    stack.count = 1;
    return true;
}

pub fn push(stack: &mut Frames, state: i32, node: i32, aux: i32) -> bool {
    if stack.count < 1 || stack.count >= 129 { return false; }
    if node < 1 || node > 128 || state < 2 || state > 21 || aux < 0 || aux > 1984 { return false; }
    let mut i = 0;
    while i < stack.count {
        if stack.node[i] == node { return false; }
        i = i + 1;
    }
    stack.state[stack.count] = state;
    stack.node[stack.count] = node;
    stack.aux[stack.count] = aux;
    stack.count = stack.count + 1;
    return true;
}

pub fn resume(stack: &mut Frames, state: i32, aux: i32) -> bool {
    if stack.count < 2 || stack.count > 129 || state < 2 || state > 21 || aux < 0 || aux > 1984 { return false; }
    stack.state[stack.count - 1] = state;
    stack.aux[stack.count - 1] = aux;
    return true;
}

pub fn pop(stack: &mut Frames) -> i32 {
    if stack.count < 2 || stack.count > 129 { return -1; }
    stack.count = stack.count - 1;
    let node = stack.node[stack.count];
    stack.node[stack.count] = 0;
    stack.state[stack.count] = 0;
    stack.aux[stack.count] = 0;
    return node;
}

pub fn fill(rows: &mut Rows) -> () {
    let mut i = 0;
    while i < 128 {
        rows.kind[i] = 2 + i % 20;
        rows.span[i] = i + 256 * (i + 1);
        rows.a[i] = i + 1;
        rows.b[i] = 128 - i;
        rows.c[i] = i % 2;
        rows.d[i] = 1 + i % 64;
        rows.next[i] = i + 2;
        i = i + 1;
    }
    rows.next[127] = 0;
    rows.count = 128;
    return;
}

pub fn valid(rows: &Rows) -> bool {
    if rows.count != 128 { return false; }
    let mut i = 0;
    while i < 128 {
        if rows.kind[i] != 2 + i % 20 || rows.span[i] != i + 256 * (i + 1) { return false; }
        if rows.a[i] != i + 1 || rows.b[i] != 128 - i || rows.c[i] != i % 2 || rows.d[i] != 1 + i % 64 { return false; }
        if i < 127 {
            if rows.next[i] != i + 2 { return false; }
        } else {
            if rows.next[i] != 0 { return false; }
        }
        i = i + 1;
    }
    return rows.kind[128] == 0 && rows.span[128] == 0 && rows.a[128] == 0 && rows.b[128] == 0 && rows.c[128] == 0 && rows.d[128] == 0 && rows.next[128] == 0;
}
