use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::len as bytes_len;
use alloc::bytes::GrowStatus;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;

#[derive(Copy)]
pub struct Token {
    pub kind: i32, pub start: i32, pub end: i32, pub value: i64,
    pub decoded_start: i32, pub decoded_length: i32
}
#[derive(Copy)]
pub struct Parameter { pub name: i32, pub ty: i32 }
#[derive(Copy)]
pub struct SourceModule {
    pub name: i32, pub first_token: i32, pub first_byte: i32, pub parent: i32,
    pub depth: i32, pub public: bool
}
#[derive(Copy)]
pub struct Import {
    pub scope: i32, pub path_start: i32, pub name: i32, pub alias: i32,
    pub target: i32, pub record_target: i32
}
#[derive(Copy)]
pub struct Function {
    pub name: i32, pub params: i32, pub count: i32, pub result: i32,
    pub begin: i32, pub end: i32, pub source_module: i32, pub public: bool
}
#[derive(Copy)]
pub struct Binding {
    pub name: i32, pub ty: i32, pub slot: i32, pub writable: bool, pub available: bool
}
#[derive(Copy)]
pub struct Loan { pub binding: i32, pub exclusive: bool }
#[derive(Copy)]
pub struct Value { pub ty: i32, pub id: i32 }
#[derive(Copy)]
pub struct Record {
    pub name: i32, pub source_module: i32, pub first_field: i32, pub field_count: i32,
    pub start: i32, pub end: i32, pub copy: bool, pub tagged: bool, pub public: bool
}
#[derive(Copy)]
pub struct Field { pub name: i32, pub ty: i32, pub public: bool }

pub struct Compiler {
    pub source: ByteBuf, pub source_path: ByteBuf, pub literals: ByteBuf, pub sources: Vec<SourceModule>, pub imports: Vec<Import>,
    pub tokens: Vec<Token>, pub params: Vec<Parameter>, pub functions: Vec<Function>,
    pub records: Vec<Record>, pub fields: Vec<Field>,
    pub vars: Vec<Binding>, pub loans: Vec<Loan>, pub loop_move_floor: i32, pub call_sites: i32,
    pub module: ByteBuf, pub slots: ByteBuf, pub body: ByteBuf,
    pub cursor: i32, pub current: i32, pub main: i32,
    pub next_value: i32, pub next_label: i32, pub next_slot: i32,
    pub target_arm64: bool,
    pub live: bool, pub terminated: bool, pub error: i32, pub error_start: i32, pub error_module: i32
}

pub fn new_compiler(source: ByteBuf, source_path: ByteBuf, target_arm64: bool) -> Compiler {
    return Compiler {
        source: source, source_path: source_path, literals: bytes_new(), sources: new::<SourceModule>(), imports: new::<Import>(),
        tokens: new::<Token>(), params: new::<Parameter>(),
        records: new::<Record>(), fields: new::<Field>(),
        functions: new::<Function>(), vars: new::<Binding>(), loans: new::<Loan>(),
        loop_move_floor: 0, call_sites: 0,
        module: bytes_new(), slots: bytes_new(), body: bytes_new(),
        cursor: 0, current: 0, main: -1, next_value: 0, next_label: 0,
        next_slot: 0, target_arm64: target_arm64,
        live: true, terminated: false, error: 0, error_start: 0, error_module: 0
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
        return Token { kind: 0, start: end, end: end, value: 0i64, decoded_start: 0, decoded_length: 0 };
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
    return same_spelling(&*c, left, right, false);
}

fn ascii_lower(value: i32) -> i32 {
    if value >= 65 && value <= 90 { return value + 32; }
    return value;
}

fn same_spelling(c: &Compiler, left: i32, right: i32, fold_case: bool) -> bool {
    let a = token(&*c, left);
    let b = token(&*c, right);
    if a.end - a.start != b.end - b.start { return false; }
    let mut i = 0;
    while i < a.end - a.start {
        let x = code(&*c.source, a.start + i);
        let y = code(&*c.source, b.start + i);
        if fold_case {
            if ascii_lower(x) != ascii_lower(y) { return false; }
        } else {
            if x != y { return false; }
        }
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

pub fn is_value_scalar(ty: i32) -> bool { return ty >= 1 && ty <= 7 && ty != 3; }

// Each scalar gets a range of 1025 tags, one per supported array length.
pub fn array_type(element: i32, length: i32) -> i32 { return 1024 + (element - 1) * 1025 + length; }
pub fn is_array(ty: i32) -> bool { return ty >= 1024 && ty < 8199; }
pub fn array_element(ty: i32) -> i32 { return (ty - 1024) / 1025 + 1; }
pub fn array_count(ty: i32) -> i32 { return (ty - 1024) % 1025; }
pub fn array_bytes(ty: i32) -> i32 {
    let element = array_element(ty);
    let length = array_count(ty);
    if element == 2 || element == 4 { return length; }
    if element == 5 { return length * 8; }
    return length * 4;
}
pub fn record_type(index: i32) -> i32 { return 8199 + index; }
pub fn is_record(ty: i32) -> bool { return ty >= 8199 && ty < 8327; }
pub fn record_index(ty: i32) -> i32 { return ty - 8199; }
pub fn slice_type(element: i32, exclusive: bool) -> i32 {
    if exclusive { return 9010 + element; }
    return 9000 + element;
}
pub fn is_mutable_slice(ty: i32) -> bool { return ty > 9010 && ty <= 9017; }
pub fn is_slice(ty: i32) -> bool { return (ty > 9000 && ty <= 9007) || is_mutable_slice(ty); }
pub fn slice_element(ty: i32) -> i32 { return ty % 10; }
pub fn record_reference(ty: i32, exclusive: bool) -> i32 {
    let index = record_index(ty);
    if exclusive { return 9228 + index; }
    return 9100 + index;
}
pub fn is_record_reference(ty: i32) -> bool { return ty >= 9100 && ty < 9356; }
pub fn is_mutable_record_reference(ty: i32) -> bool { return ty >= 9228 && ty < 9356; }
pub fn referent_record(ty: i32) -> i32 {
    return record_type((ty - 9100) % 128);
}

pub fn record(c: &Compiler, source_module: i32, name: i32) -> i32 {
    let mut index = 0;
    while index < len(&*c.records) {
        let row = c.records[index];
        if row.source_module == source_module && same_name(&*c, row.name, name) { return index; }
        index = index + 1;
    }
    return -1;
}

pub fn record_field(c: &Compiler, row: Record, name: i32) -> i32 {
    let mut index = 0;
    while index < row.field_count {
        let field = c.fields[row.first_field + index];
        if same_name(&*c, field.name, name) { return index; }
        index = index + 1;
    }
    return -1;
}

pub fn function(c: &Compiler, source_module: i32, name: i32) -> i32 {
    let mut i = 0;
    while i < len(&*c.functions) {
        let f = c.functions[i];
        if f.source_module == source_module && same_name(&*c, f.name, name) { return i; }
        i = i + 1;
    }
    return -1;
}

fn within(c: &Compiler, module: i32, ancestor: i32) -> bool {
    let mut current = module;
    while current >= 0 {
        if current == ancestor { return true; }
        let row = c.sources[current];
        current = row.parent;
    }
    return false;
}

pub fn record_visible(c: &Compiler, requester: i32, index: i32) -> bool {
    if index < 0 { return false; }
    let row = c.records[index];
    return row.public || within(&*c, requester, row.source_module);
}

pub fn record_binding(c: &Compiler, source_module: i32, name: i32) -> i32 {
    let declared = record(&*c, source_module, name);
    if declared >= 0 { return declared; }
    for index in 0..len(&*c.imports) {
        let imported = c.imports[index];
        if imported.scope == source_module && imported.record_target >= 0
            && same_name(&*c, imported.alias, name) { return imported.record_target; }
    }
    return -1;
}

pub fn visible_field(c: &Compiler, requester: i32, row: Record, name: i32) -> i32 {
    let position = record_field(&*c, row, name);
    if position < 0 { return -1; }
    let field = c.fields[row.first_field + position];
    if field.public || within(&*c, requester, row.source_module) { return position; }
    return -1;
}

pub fn function_visible(c: &Compiler, source_module: i32, target: i32) -> bool {
    if target < 0 { return false; }
    let selected = c.functions[target];
    return selected.public || within(&*c, source_module, selected.source_module);
}

pub fn function_binding(c: &Compiler, source_module: i32, name: i32) -> i32 {
    let declared = function(&*c, source_module, name);
    if declared >= 0 { return declared; }
    let mut index = 0;
    while index < len(&*c.imports) {
        let imported = c.imports[index];
        if imported.scope == source_module && imported.target >= 0
            && same_name(&*c, imported.alias, name) { return imported.target; }
        index = index + 1;
    }
    return -1;
}

pub fn child_module(c: &Compiler, parent: i32, name: i32, fold_case: bool) -> i32 {
    let mut index = 1;
    while index < len(&*c.sources) {
        let source_module = c.sources[index];
        if source_module.parent == parent && same_spelling(&*c, source_module.name, name, fold_case) {
            return index;
        }
        index = index + 1;
    }
    return -1;
}

pub fn path_module(c: &Compiler, start: i32, function_name: i32, requester: i32) -> i32 {
    let mut parent = 0;
    let mut component = start;
    while component < function_name {
        let child = child_module(&*c, parent, component, false);
        if child < 0 { return -1; }
        let row = c.sources[child];
        if !row.public && !within(&*c, requester, parent) { return -1; }
        parent = child;
        component = component + 2;
    }
    return parent;
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
