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
pub struct Loan { pub binding: i32, pub path: i64, pub depth: i32, pub exclusive: bool }
#[derive(Copy)]
pub struct Value { pub ty: i32, pub id: i32 }
#[derive(Copy)]
pub struct Record {
    pub name: i32, pub source_module: i32, pub first_field: i32, pub field_count: i32,
    pub start: i32, pub body: i32, pub end: i32, pub cells: i32, pub depth: i32,
    pub layout_state: i32,
    pub vector_element: i32,
    pub copy: bool, pub tagged: bool, pub public: bool, pub needs_drop: bool
}
#[derive(Copy)]
pub struct Field { pub name: i32, pub ty: i32, pub public: bool }
#[derive(Copy)]
pub struct FieldPath { pub code: i64, pub depth: i32, pub ty: i32, pub pointer: i32 }
#[derive(Copy)]
pub struct RecordShape { pub width: i32, pub alignment: i32, pub dense: i32, pub leaves: i32 }

pub struct Compiler {
    pub source: ByteBuf, pub source_path: ByteBuf, pub literals: ByteBuf, pub sources: Vec<SourceModule>, pub imports: Vec<Import>,
    pub tokens: Vec<Token>, pub params: Vec<Parameter>, pub functions: Vec<Function>,
    pub records: Vec<Record>, pub fields: Vec<Field>,
    pub vars: Vec<Binding>, pub loans: Vec<Loan>, pub loop_move_floor: i32, pub call_sites: i32,
    pub module: ByteBuf, pub slots: ByteBuf, pub body: ByteBuf,
    pub cursor: i32, pub current: i32, pub main: i32,
    pub next_value: i32, pub next_label: i32, pub next_slot: i32,
    pub target: i32, pub process_entry: bool, pub uses_argument_count: bool,
    pub uses_argument_bytes: bool, pub argument_status: i32,
    pub uses_binary_io: bool, pub read_status: i32, pub write_status: i32, pub uses_time: bool,
    pub byte_buffer: i32, pub growth_status: i32,
    pub file_owner: i32, pub file_open_result: i32, pub file_status: i32,
    pub vector_types: Vec<i32>,
    pub uses_record_slices: bool,
    pub live: bool, pub terminated: bool, pub error: i32, pub error_start: i32, pub error_module: i32
}

pub fn new_compiler(source: ByteBuf, source_path: ByteBuf, target: i32, process_entry: bool) -> Compiler {
    return Compiler {
        source: source, source_path: source_path, literals: bytes_new(), sources: new::<SourceModule>(), imports: new::<Import>(),
        tokens: new::<Token>(), params: new::<Parameter>(),
        records: new::<Record>(), fields: new::<Field>(),
        functions: new::<Function>(), vars: new::<Binding>(), loans: new::<Loan>(),
        loop_move_floor: 0, call_sites: 0,
        module: bytes_new(), slots: bytes_new(), body: bytes_new(),
        cursor: 0, current: 0, main: -1, next_value: 0, next_label: 0,
        next_slot: 0, target: target, process_entry: process_entry, uses_argument_count: false,
        uses_argument_bytes: false, argument_status: -1,
        uses_binary_io: false, read_status: -1, write_status: -1, uses_time: false,
        byte_buffer: -1, growth_status: -1,
        file_owner: -1, file_open_result: -1, file_status: -1,
        vector_types: new::<i32>(),
        uses_record_slices: false,
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

pub fn source_byte_limit() -> i32 { return 524288; }
pub fn source_token_limit() -> i32 { return 131072; }

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
    if is_record(element) {
        if exclusive { return 9528 + record_index(element); }
        return 9400 + record_index(element);
    }
    if exclusive { return 9010 + element; }
    return 9000 + element;
}
pub fn is_mutable_slice(ty: i32) -> bool {
    return (ty > 9010 && ty <= 9017) || (ty >= 9528 && ty < 9656);
}
pub fn is_record_slice(ty: i32) -> bool { return ty >= 9400 && ty < 9656; }
pub fn is_slice(ty: i32) -> bool { return (ty > 9000 && ty <= 9007) || is_mutable_slice(ty) || is_record_slice(ty); }
pub fn slice_element(ty: i32) -> i32 {
    if is_record_slice(ty) { return record_type((ty - 9400) % 128); }
    return ty % 10;
}
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
pub fn record_value_type(ty: i32) -> i32 {
    if is_record_reference(ty) { return referent_record(ty); }
    return ty;
}

pub fn value_cells(c: &Compiler, ty: i32) -> i32 {
    if is_record(ty) {
        let row = c.records[record_index(ty)];
        return row.cells;
    }
    if is_slice(ty) { return 2; }
    if is_array(ty) {
        let cells = (array_bytes(ty) + 7) / 8;
        if cells > 0 { return cells; }
    }
    return 1;
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
        if field.name >= 0 {
            if same_name(&*c, field.name, name) { return index; }
        } else if standard_variant(&*c, field.name, name) { return index; }
        index = index + 1;
    }
    return -1;
}

fn standard_variant(c: &Compiler, variant_id: i32, name: i32) -> bool {
    let spelling = token(&*c, name);
    if variant_id == -1 {
        let data = b"Data";
        return word(&*c.source, spelling.start, spelling.end, &data);
    }
    if variant_id == -2 {
        let eof = b"Eof";
        return word(&*c.source, spelling.start, spelling.end, &eof);
    }
    if variant_id == -3 {
        let error_name = b"Error";
        return word(&*c.source, spelling.start, spelling.end, &error_name);
    }
    if variant_id == -4 {
        let written = b"Written";
        return word(&*c.source, spelling.start, spelling.end, &written);
    }
    if variant_id == -5 {
        let copied = b"Copied";
        return word(&*c.source, spelling.start, spelling.end, &copied);
    }
    if variant_id == -6 {
        let too_small = b"TooSmall";
        return word(&*c.source, spelling.start, spelling.end, &too_small);
    }
    if variant_id == -7 {
        let missing = b"Missing";
        return word(&*c.source, spelling.start, spelling.end, &missing);
    }
    if variant_id == -8 {
        let too_long = b"TooLong";
        return word(&*c.source, spelling.start, spelling.end, &too_long);
    }
    if variant_id == -9 {
        let ok = b"Ok";
        return word(&*c.source, spelling.start, spelling.end, &ok);
    }
    if variant_id == -10 {
        let capacity = b"CapacityOverflow";
        return word(&*c.source, spelling.start, spelling.end, &capacity);
    }
    if variant_id == -11 {
        let allocation = b"AllocationFailed";
        return word(&*c.source, spelling.start, spelling.end, &allocation);
    }
    if variant_id == -12 {
        let memory = b"MemoryLimit";
        return word(&*c.source, spelling.start, spelling.end, &memory);
    }
    let opened = b"Opened";
    return variant_id == -13 && word(&*c.source, spelling.start, spelling.end, &opened);
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
        if imported.scope == source_module && imported.target != -1
            && same_name(&*c, imported.alias, name) { return imported.target; }
        index = index + 1;
    }
    return -1;
}

pub fn argument_count_binding() -> i32 { return -2; }
pub fn read_some_binding() -> i32 { return -3; }
pub fn write_some_binding() -> i32 { return -4; }
pub fn write_stderr_binding() -> i32 { return -5; }
pub fn read_status_binding() -> i32 { return -6; }
pub fn write_status_binding() -> i32 { return -7; }
pub fn argument_bytes_binding() -> i32 { return -8; }
pub fn argument_status_binding() -> i32 { return -9; }
pub fn byte_buffer_binding() -> i32 { return -10; }
pub fn growth_status_binding() -> i32 { return -11; }
pub fn byte_operation_binding(operation: i32) -> i32 { return -12 - operation; }
pub fn is_byte_operation(binding: i32) -> bool {
    return (binding <= byte_operation_binding(0) && binding >= byte_operation_binding(7))
        || binding == byte_operation_binding(10);
}
pub fn is_byte_buffer(c: &Compiler, ty: i32) -> bool {
    return c.byte_buffer >= 0 && ty == record_type(c.byte_buffer);
}
pub fn file_owner_binding() -> i32 { return -23; }
pub fn file_open_result_binding() -> i32 { return -24; }
pub fn file_status_binding() -> i32 { return -25; }
pub fn file_open_read_binding() -> i32 { return -26; }
pub fn file_read_binding() -> i32 { return -27; }
pub fn file_close_binding() -> i32 { return -28; }
pub fn file_create_binding() -> i32 { return -29; }
pub fn file_append_binding() -> i32 { return -30; }
pub fn file_write_binding() -> i32 { return -31; }
pub fn file_sync_binding() -> i32 { return -32; }
pub fn file_seek_binding() -> i32 { return -33; }
pub fn is_file_operation(binding: i32) -> bool {
    return binding >= file_seek_binding() && binding <= file_open_read_binding();
}
pub fn file_open_operation(binding: i32) -> i32 {
    if binding == file_open_read_binding() { return 0; }
    if binding == file_create_binding() { return 1; }
    if binding == file_append_binding() { return 6; }
    return -1;
}
pub fn file_transfer_operation(binding: i32) -> i32 {
    if binding == file_read_binding() { return 2; }
    if binding == file_write_binding() { return 3; }
    if binding == file_sync_binding() { return 4; }
    return 5;
}
pub fn is_file(c: &Compiler, ty: i32) -> bool {
    return c.file_owner >= 0 && ty == record_type(c.file_owner);
}
pub fn uses_heap(c: &Compiler) -> bool {
    return c.byte_buffer >= 0 || c.file_owner >= 0 || len(&*c.vector_types) > 0 || c.uses_record_slices;
}
pub fn vector_type_binding() -> i32 { return -34; }
pub fn vector_operation_binding(operation: i32) -> i32 { return -35 - operation; }
pub fn is_vector_operation(binding: i32) -> bool {
    return binding <= vector_operation_binding(0) && binding >= vector_operation_binding(8);
}
pub fn monotonic_millis_binding() -> i32 { return -44; }
pub fn unix_millis_binding() -> i32 { return -45; }
pub fn sleep_millis_binding() -> i32 { return -46; }
pub fn is_time_operation(binding: i32) -> bool {
    return binding >= sleep_millis_binding() && binding <= monotonic_millis_binding();
}
pub fn vector_type(c: &Compiler, element: i32) -> i32 {
    let mut slot = element;
    if is_record(element) { slot = 8 + record_index(element); }
    if slot <= 0 || slot >= len(&*c.vector_types) { return 0; }
    let index = c.vector_types[slot];
    if index < 0 { return 0; }
    return record_type(index);
}
pub fn vector_element(c: &Compiler, ty: i32) -> i32 {
    if !is_record(ty) { return 0; }
    let row = c.records[record_index(ty)];
    return row.vector_element;
}
pub fn vector_runtime_tag(element: i32) -> i32 {
    if is_record(element) { return -2147483648 + record_index(element); }
    if element == 1 { return 7; }
    if element == 2 { return 1; }
    if element == 4 { return 4; }
    if element == 5 { return 9; }
    if element == 6 { return 15; }
    if element == 7 { return 8; }
    if element == 3 { return 2; }
    return 0;
}
pub fn aligned_width(width: i32, alignment: i32) -> i32 {
    return ((width + alignment - 1) / alignment) * alignment;
}
pub fn copy_record_shape(c: &Compiler, ty: i32) -> RecordShape {
    if !is_record(ty) {
        let mut width = 4;
        if ty == 2 || ty == 3 || ty == 4 { width = 1; }
        if ty == 5 { width = 8; }
        return RecordShape { width: width, alignment: width, dense: width, leaves: 1 };
    }
    let row = c.records[record_index(ty)];
    if row.field_count == 0 { return RecordShape { width: 1, alignment: 1, dense: 1, leaves: 1 }; }
    let mut shape = RecordShape { width: 0, alignment: 1, dense: 0, leaves: 0 };
    for index in 0..row.field_count {
        let field = c.fields[row.first_field + index];
        let child = copy_record_shape(&*c, field.ty);
        shape.width = aligned_width(shape.width, child.alignment) + child.width;
        if child.alignment > shape.alignment { shape.alignment = child.alignment; }
        shape.dense = shape.dense + child.dense;
        shape.leaves = shape.leaves + child.leaves;
    }
    shape.width = aligned_width(shape.width, shape.alignment);
    return shape;
}
pub fn needs_drop(c: &Compiler, ty: i32) -> bool {
    if !is_record(ty) { return false; }
    let row = c.records[record_index(ty)];
    return row.needs_drop;
}

pub fn is_binary_io_binding(binding: i32) -> bool { return binding >= -5 && binding <= -3; }

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
