use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::len as bytes_len;
use alloc::bytes::GrowStatus;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;

#[derive(Copy)]
pub struct Token { pub kind: i32, pub start: i32, pub end: i32, pub value: i32 }
#[derive(Copy)]
pub struct Parameter { pub name: i32, pub ty: i32 }
#[derive(Copy)]
pub struct Function {
    pub name: i32, pub params: i32, pub count: i32, pub result: i32,
    pub begin: i32, pub end: i32
}
#[derive(Copy)]
pub struct Binding { pub name: i32, pub ty: i32, pub slot: i32, pub writable: bool }
#[derive(Copy)]
pub struct Edge { pub caller: i32, pub callee: i32 }
#[derive(Copy)]
pub struct Value { pub ty: i32, pub id: i32 }

pub struct Compiler {
    pub source: ByteBuf,
    pub tokens: Vec<Token>, pub params: Vec<Parameter>, pub functions: Vec<Function>,
    pub vars: Vec<Binding>, pub edges: Vec<Edge>,
    pub module: ByteBuf, pub slots: ByteBuf, pub body: ByteBuf,
    pub cursor: i32, pub current: i32, pub main: i32,
    pub next_value: i32, pub next_label: i32, pub next_slot: i32,
    pub live: bool, pub terminated: bool, pub error: i32, pub error_start: i32
}

pub fn new_compiler(source: ByteBuf) -> Compiler {
    return Compiler {
        source: source, tokens: new::<Token>(), params: new::<Parameter>(),
        functions: new::<Function>(), vars: new::<Binding>(), edges: new::<Edge>(),
        module: bytes_new(), slots: bytes_new(), body: bytes_new(),
        cursor: 0, current: 0, main: -1, next_value: 0, next_label: 0,
        next_slot: 0, live: true, terminated: false, error: 0, error_start: 0
    };
}

pub fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}

pub fn token(c: &Compiler, index: i32) -> Token {
    if index < 0 || index >= len(&*c.tokens) {
        let end = bytes_len(&*c.source);
        return Token { kind: 0, start: end, end: end, value: 0 };
    }
    return c.tokens[index];
}

pub fn kind(c: &Compiler) -> i32 {
    let current = token(&*c, c.cursor);
    return current.kind;
}

pub fn fail(c: &mut Compiler) -> () {
    if c.error == 0 {
        let t = token(&*c, c.cursor);
        c.error = 65;
        c.error_start = t.start;
    }
    return;
}

pub fn expect(c: &mut Compiler, wanted: i32) -> () {
    if c.error != 0 { return; }
    if kind(&*c) != wanted { fail(&mut *c); return; }
    c.cursor = c.cursor + 1;
    return;
}

pub fn same_name(c: &Compiler, left: i32, right: i32) -> bool {
    let a = token(&*c, left);
    let b = token(&*c, right);
    if a.end - a.start != b.end - b.start { return false; }
    let mut i = 0;
    while i < a.end - a.start {
        if code(&*c.source, a.start + i) != code(&*c.source, b.start + i) { return false; }
        i = i + 1;
    }
    return true;
}

pub fn word(source: &[u8], start: i32, end: i32, text: &[u8]) -> bool {
    if end - start != text.len() { return false; }
    let mut i = 0;
    while i < text.len() {
        if source[start + i] != text[i] { return false; }
        i = i + 1;
    }
    return true;
}

pub fn code(bytes: &[u8], index: i32) -> i32 {
    let selected = bytes[index];
    return selected.to_i32();
}

pub fn set_byte(bytes: &mut [u8], index: i32, value: i32) -> () {
    bytes[index] = value.to_u8_checked();
    return;
}

pub fn function(c: &Compiler, name: i32) -> i32 {
    let mut i = 0;
    while i < len(&*c.functions) {
        let f = c.functions[i];
        if same_name(&*c, f.name, name) { return i; }
        i = i + 1;
    }
    return -1;
}

pub fn local(c: &Compiler, name: i32) -> i32 {
    let mut i = len(&*c.vars);
    while i > 0 {
        i = i - 1;
        let v = c.vars[i];
        if same_name(&*c, v.name, name) { return i; }
    }
    return -1;
}

pub fn edge(c: &mut Compiler, callee: i32) -> () {
    if len(&*c.edges) >= 4096 { c.error = 68; return; }
    let row = Edge { caller: c.current, callee: callee };
    if !grown(try_push(&mut *c.edges, row)) { c.error = 68; }
    return;
}
