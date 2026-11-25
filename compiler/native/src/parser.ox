use crate::model::Compiler;
use crate::model::Value;
use crate::model::Token;
use crate::model::Binding;
use crate::model::Loan;
use crate::model::Record;
use crate::model::grown;
use crate::model::token;
use crate::model::kind;
use crate::model::expect;
use crate::model::fail;
use crate::model::local;
use crate::model::function;
use crate::model::function_binding;
use crate::model::argument_count_binding;
use crate::model::argument_bytes_binding;
use crate::model::byte_operation_binding;
use crate::model::is_byte_operation;
use crate::model::is_byte_buffer;
use crate::model::needs_drop;
use crate::model::record_reference;
use crate::model::is_binary_io_binding;
use crate::model::read_some_binding;
use crate::model::function_visible;
use crate::model::path_module;
use crate::model::is_array;
use crate::model::array_count;
use crate::model::array_element;
use crate::model::is_value_scalar;
use crate::model::array_type;
use crate::model::is_slice;
use crate::model::is_mutable_slice;
use crate::model::slice_type;
use crate::model::slice_element;
use crate::model::is_record_reference;
use crate::model::is_mutable_record_reference;
use crate::model::referent_record;
use crate::model::record_value_type;
use crate::model::FieldPath;
use crate::model::record;
use crate::model::record_binding;
use crate::model::record_type;
use crate::model::record_index;
use crate::model::is_record;
use crate::model::record_field;
use crate::model::record_visible;
use crate::model::visible_field;
use crate::model::word;
use crate::headers::ty as parse_type;
use crate::headers::path_name;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;
use alloc::vec::truncate;

fn invalid() -> Value { return Value { ty: 0, id: 0 }; }
fn integer_type(ty: i32) -> bool { return ty == 1 || ty == 4 || ty == 5 || ty == 7; }
fn precedence(op: i32) -> i32 {
    if op == 42 { return 1; }
    if op == 41 { return 2; }
    if op == 35 || op == 36 { return 3; }
    if op >= 37 && op <= 40 { return 4; }
    if op == 66 { return 5; }
    if op == 67 { return 6; }
    if op == 65 { return 7; }
    if op == 69 || op == 70 { return 8; }
    if op == 29 || op == 30 { return 9; }
    if op >= 31 && op <= 33 { return 10; }
    return 0;
}
fn infix(c: &Compiler) -> i32 {
    let current = token(&*c, c.cursor);
    if current.kind == 37 || current.kind == 39 {
        let next = token(&*c, c.cursor + 1);
        if current.kind == next.kind && current.end == next.start {
            if current.kind == 37 { return 69; }
            return 70;
        }
    }
    return current.kind;
}
fn bind(c: &mut Compiler, name: i32, ty: i32, writable: bool, slot: i32) -> () {
    let owner = c.functions[c.current];
    if local(&*c, name) >= 0 || function_binding(&*c, owner.source_module, name) != -1 { fail(&mut *c); return; }
    if len(&*c.vars) >= 4096 { c.error = 68; return; }
    let row = Binding { name: name, ty: ty, slot: slot, writable: writable, available: true };
    if !grown(try_push(&mut *c.vars, row)) { c.error = 68; }
    return;
}
fn set_available(c: &mut Compiler, position: i32, available: bool) -> () {
    let mut binding = c.vars[position];
    binding.available = available;
    c.vars[position] = binding;
    return;
}
fn availability(c: &mut Compiler) -> Vec<bool> {
    let mut result = new::<bool>();
    let mut index = 0;
    while index < len(&*c.vars) && c.error == 0 {
        let binding = c.vars[index];
        if !grown(try_push(&mut result, binding.available)) { c.error = 68; }
        index = index + 1;
    }
    return result;
}
fn restore_availability(c: &mut Compiler, state: &Vec<bool>) -> () {
    let mut index = 0;
    while index < len(&*state) && c.error == 0 {
        set_available(&mut *c, index, state[index]);
        index = index + 1;
    }
    return;
}
fn intersect_availability(c: &mut Compiler, previous: &Vec<bool>) -> () {
    for index in 0..len(&*previous) {
        let binding = c.vars[index];
        let available = binding.available && previous[index];
        set_available(&mut *c, index, available);
    }
    return;
}
// Each field occupies one base-64 digit. Drop the extra digits on a deeper
// path to compare the common prefix, including the whole-owner prefix.
fn paths_overlap(a: i64, a_depth: i32, b: i64, b_depth: i32) -> bool {
    let mut left = a;
    let mut right = b;
    let mut difference = a_depth - b_depth;
    while difference > 0 { left = left / 64i64; difference = difference - 1; }
    while difference < 0 { right = right / 64i64; difference = difference + 1; }
    return left == right;
}
fn access_allowed(c: &Compiler, position: i32, path: i64, depth: i32, exclusive: bool) -> bool {
    for index in 0..len(&*c.loans) {
        let loan = c.loans[index];
        if loan.binding == position && (exclusive || loan.exclusive)
            && paths_overlap(path, depth, loan.path, loan.depth) { return false; }
    }
    return true;
}
fn slice_owner_matches(c: &Compiler, binding: Binding, expected: i32, reborrow: bool) -> bool {
    let exclusive = is_mutable_slice(expected);
    if is_byte_buffer(&*c, record_value_type(binding.ty)) {
        let reference = is_record_reference(binding.ty);
        let mut writable = binding.writable;
        if reference { writable = is_mutable_record_reference(binding.ty); }
        return expected == slice_type(4, exclusive) && reborrow == reference && (!exclusive || writable);
    }
    if reborrow {
        return is_slice(binding.ty) && slice_element(binding.ty) == slice_element(expected)
            && (!exclusive || is_mutable_slice(binding.ty));
    }
    return is_array(binding.ty) && slice_type(array_element(binding.ty), exclusive) == expected
        && (!exclusive || binding.writable);
}
fn slice_owner_view(c: &mut Compiler, binding: Binding, path: FieldPath, expected: i32) -> Value {
    if path.depth > 0 {
        if is_byte_buffer(&*c, path.ty) {
            let ptr_type = record_reference(path.ty, is_mutable_slice(expected));
            return crate::emit::byte_buffer_view(&mut *c, Value { ty: ptr_type, id: path.pointer }, expected);
        }
        return crate::emit::borrow_array_field(&mut *c, path.pointer, path.ty, expected);
    }
    if is_byte_buffer(&*c, record_value_type(binding.ty)) {
        let ptr_type = record_reference(record_value_type(binding.ty), is_mutable_slice(expected));
        let buffer = crate::emit::borrow_record(&mut *c, binding, ptr_type);
        return crate::emit::byte_buffer_view(&mut *c, buffer, expected);
    }
    if is_slice(binding.ty) {
        let loaded = crate::emit::load(&mut *c, binding.slot, binding.ty);
        return Value { ty: expected, id: loaded.id };
    }
    return crate::emit::borrow_array(&mut *c, binding, expected);
}
fn is_slice_field(c: &Compiler, ty: i32, expected: i32, exclusive: bool) -> bool {
    if is_array(ty) { return slice_type(array_element(ty), exclusive) == expected; }
    return is_byte_buffer(&*c, ty) && expected == slice_type(4, exclusive);
}
fn slice_argument(c: &mut Compiler, expected: i32, depth: i32) -> Value {
    expect(&mut *c, 65);
    let exclusive = kind(&*c) == 5;
    if exclusive { expect(&mut *c, 5); }
    if exclusive != is_mutable_slice(expected) { fail(&mut *c); return invalid(); }
    let reborrow = kind(&*c) == 31;
    if reborrow { expect(&mut *c, 31); }
    let position = local(&*c, c.cursor);
    expect(&mut *c, 1);
    if position < 0 || c.error != 0 { fail(&mut *c); return invalid(); }
    let binding = c.vars[position];
    let mut path = FieldPath { code: 1i64, depth: 0, ty: binding.ty, pointer: -1 };
    if kind(&*c) == 43 {
        let target = record_value_type(binding.ty);
        let reference = is_record_reference(binding.ty);
        let mut writable = binding.writable;
        if reference { writable = is_mutable_record_reference(binding.ty); }
        if !is_record(target) || reborrow != reference || (exclusive && !writable) {
            fail(&mut *c); return invalid();
        }
        path = field_path(&mut *c, binding);
        if c.error != 0 || !is_slice_field(&*c, path.ty, expected, exclusive) {
            fail(&mut *c); return invalid();
        }
    } else if !slice_owner_matches(&*c, binding, expected, reborrow) {
        fail(&mut *c); return invalid();
    }
    let projected = kind(&*c) == 51;
    let mut start = invalid();
    let mut end = invalid();
    if projected {
        expect(&mut *c, 51);
        start = expression(&mut *c, 1, depth + 1);
        expect(&mut *c, 57);
        end = expression(&mut *c, 1, depth + 1);
        expect(&mut *c, 52);
        if start.ty != 1 || end.ty != 1 { fail(&mut *c); return invalid(); }
    }
    let target = c.vars[position];
    if c.error != 0 || !target.available || !access_allowed(&*c, position, path.code, path.depth, exclusive) {
        fail(&mut *c); return invalid();
    }
    if !grown(try_push(&mut *c.loans, Loan { binding: position, path: path.code, depth: path.depth, exclusive: exclusive })) {
        c.error = 68; return invalid();
    }
    let view = slice_owner_view(&mut *c, binding, path, expected);
    if projected { return crate::emit::project_slice(&mut *c, view, start, end); }
    return view;
}
fn record_argument(c: &mut Compiler, expected: i32) -> Value {
    expect(&mut *c, 65);
    let exclusive = kind(&*c) == 5;
    if exclusive { expect(&mut *c, 5); }
    if exclusive != is_mutable_record_reference(expected) { fail(&mut *c); return invalid(); }
    let reborrow = kind(&*c) == 31;
    if reborrow { expect(&mut *c, 31); }
    let position = local(&*c, c.cursor);
    expect(&mut *c, 1);
    if position < 0 || c.error != 0 { fail(&mut *c); return invalid(); }
    let binding = c.vars[position];
    let root = record_value_type(binding.ty);
    let reference = is_record_reference(binding.ty);
    let mut writable = binding.writable;
    if reference { writable = is_mutable_record_reference(binding.ty); }
    if !is_record(root) || reborrow != reference || (exclusive && !writable) {
        fail(&mut *c); return invalid();
    }
    let mut path = FieldPath { code: 1i64, depth: 0, ty: root, pointer: -1 };
    if kind(&*c) == 43 { path = field_path(&mut *c, binding); }
    if c.error != 0 || path.ty != referent_record(expected) || !binding.available
        || !access_allowed(&*c, position, path.code, path.depth, exclusive) {
        fail(&mut *c); return invalid();
    }
    if !grown(try_push(&mut *c.loans, Loan { binding: position, path: path.code, depth: path.depth, exclusive: exclusive })) {
        c.error = 68; return invalid();
    }
    if path.depth > 0 { return Value { ty: expected, id: path.pointer }; }
    return crate::emit::borrow_record(&mut *c, binding, expected);
}
fn byte_buffer_call(c: &mut Compiler, operation: i32, depth: i32) -> Value {
    expect(&mut *c, 20);
    let ty = record_type(c.byte_buffer);
    if operation == 0 {
        expect(&mut *c, 21);
        return Value { ty: ty, id: -1 };
    }
    let outer_loans = len(&*c.loans);
    let borrow_type = record_reference(ty, operation >= 3);
    let buffer = record_argument(&mut *c, borrow_type);
    let mut size = invalid();
    let mut fill = invalid();
    let mut view = invalid();
    if operation == 3 || operation == 6 || operation == 7 {
        expect(&mut *c, 26);
        size = expression(&mut *c, 1, depth + 1);
        if size.ty != 1 { fail(&mut *c); return invalid(); }
    }
    if operation == 4 || operation == 6 {
        expect(&mut *c, 26);
        fill = expression(&mut *c, 1, depth + 1);
        if fill.ty != 4 { fail(&mut *c); return invalid(); }
    }
    if operation == 5 {
        expect(&mut *c, 26);
        view = slice_argument(&mut *c, slice_type(4, false), depth);
    }
    expect(&mut *c, 21);
    truncate(&mut *c.loans, outer_loans);
    if c.error != 0 { return invalid(); }
    return crate::emit::byte_buffer_operation(&mut *c, operation, buffer, size, fill, view);
}
fn called(c: &mut Compiler, target: i32, depth: i32) -> Value {
    if is_byte_operation(target) {
        return byte_buffer_call(&mut *c, byte_operation_binding(0) - target, depth);
    }
    if target == argument_count_binding() {
        expect(&mut *c, 20); expect(&mut *c, 21);
        if c.error != 0 { return invalid(); }
        return crate::emit::argument_count(&mut *c);
    }
    if target == argument_bytes_binding() {
        let outer_loans = len(&*c.loans);
        expect(&mut *c, 20);
        let index = expression(&mut *c, 1, depth + 1);
        if index.ty != 1 { fail(&mut *c); return invalid(); }
        expect(&mut *c, 26);
        let view = slice_argument(&mut *c, slice_type(4, true), depth);
        expect(&mut *c, 21);
        truncate(&mut *c.loans, outer_loans);
        if c.error != 0 { return invalid(); }
        return crate::emit::argument_bytes(&mut *c, index, view);
    }
    if is_binary_io_binding(target) {
        let reading = target == read_some_binding();
        let outer_loans = len(&*c.loans);
        expect(&mut *c, 20);
        let view = slice_argument(&mut *c, slice_type(4, reading), depth);
        expect(&mut *c, 21);
        truncate(&mut *c.loans, outer_loans);
        if c.error != 0 { return invalid(); }
        let mut status = c.write_status;
        if reading { status = c.read_status; }
        return crate::emit::binary_io(&mut *c, target, view, status);
    }
    if target < 0 { fail(&mut *c); return invalid(); }
    let f = c.functions[target];
    let mut args = new::<Value>();
    let outer_loans = len(&*c.loans);
    expect(&mut *c, 20);
    let mut count = 0;
    while kind(&*c) != 21 && c.error == 0 {
        if count >= f.count { fail(&mut *c); return invalid(); }
        let expected = c.params[f.params + count];
        let mut value = invalid();
        if is_slice(expected.ty) {
            value = slice_argument(&mut *c, expected.ty, depth);
        } else if is_record_reference(expected.ty) {
            value = record_argument(&mut *c, expected.ty);
        } else {
            value = expression(&mut *c, 1, depth + 1);
        }
        if value.ty != expected.ty { fail(&mut *c); return invalid(); }
        if !grown(try_push(&mut args, value)) { c.error = 68; return invalid(); }
        count = count + 1;
        if kind(&*c) != 21 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 21);
    if count != f.count { fail(&mut *c); return invalid(); }
    // Nested calls keep the loans of their enclosing argument list.
    truncate(&mut *c.loans, outer_loans);
    if c.call_sites >= 4096 { c.error = 68; return invalid(); }
    c.call_sites = c.call_sites + 1;
    return crate::emit::call(&mut *c, target, &args, f.result);
}
fn qualified_enum(c: &Compiler, first: i32, last: i32, requester: i32) -> i32 {
    if last < first + 2 { return -1; }
    let type_name = last - 2;
    let owner_module = path_module(&*c, first, type_name, requester);
    if owner_module < 0 { return -1; }
    let index = record(&*c, owner_module, type_name);
    if !record_visible(&*c, requester, index) { return -1; }
    let row = c.records[index];
    if !row.tagged { return -1; }
    return index;
}
fn qualified_value(c: &mut Compiler, depth: i32) -> Value {
    let path_start = c.cursor + 2;
    let name = path_name(&mut *c);
    if c.error != 0 { return invalid(); }
    let caller = c.functions[c.current];
    let enumeration = qualified_enum(&*c, path_start, name, caller.source_module);
    if enumeration >= 0 { return enum_value(&mut *c, enumeration, name, depth); }
    let source_module = path_module(&*c, path_start, name, caller.source_module);
    if source_module < 0 { fail(&mut *c); return invalid(); }
    if kind(&*c) == 22 {
        let record_id = record(&*c, source_module, name);
        if !record_visible(&*c, caller.source_module, record_id) { fail(&mut *c); return invalid(); }
        return record_literal(&mut *c, record_id, depth);
    }
    let target = function(&*c, source_module, name);
    if !function_visible(&*c, caller.source_module, target) { fail(&mut *c); return invalid(); }
    return called(&mut *c, target, depth);
}
fn as_long(c: &mut Compiler, value: Value) -> Value {
    if value.ty == 5 { return value; }
    return crate::emit::widen(&mut *c, value, 5);
}
fn extended_conversion(c: &mut Compiler, value: Value, method: Token) -> Value {
    let to_char = b"to_char_checked";
    let code_point = b"to_u32";
    let to_unsigned = b"to_u32_checked";
    let to_long = b"to_i64_checked";
    if value.ty == 6 && word(&*c.source, method.start, method.end, &code_point) {
        return Value { ty: 7, id: value.id };
    }
    if integer_type(value.ty) {
        if word(&*c.source, method.start, method.end, &to_char) {
            let number = as_long(&mut *c, value);
            return crate::emit::checked_character(&mut *c, number);
        }
        if word(&*c.source, method.start, method.end, &to_unsigned) {
            if value.ty == 7 { return value; }
            let number = as_long(&mut *c, value);
            return crate::emit::narrow_unsigned(&mut *c, number, false);
        }
        if word(&*c.source, method.start, method.end, &to_long) {
            return as_long(&mut *c, value);
        }
    }
    fail(&mut *c); return invalid();
}
fn convert_binding(c: &mut Compiler, value: Value) -> Value {
    let narrow_byte = b"to_u8_checked";
    let byte_to_int = b"to_i32";
    let to_long = b"to_i64";
    let to_int = b"to_i32_checked";
    expect(&mut *c, 43);
    let method = token(&*c, c.cursor);
    expect(&mut *c, 1);
    expect(&mut *c, 20);
    expect(&mut *c, 21);
    if c.error != 0 { return invalid(); }
    if value.ty == 4 && word(&*c.source, method.start, method.end, &byte_to_int) {
        return crate::emit::widen(&mut *c, value, 1);
    }
    if word(&*c.source, method.start, method.end, &to_long) {
        if value.ty == 1 || value.ty == 4 { return crate::emit::widen(&mut *c, value, 5); }
    }
    if word(&*c.source, method.start, method.end, &to_int) {
        if value.ty == 1 { return value; }
        if value.ty == 4 { return crate::emit::widen(&mut *c, value, 1); }
        if value.ty == 5 { return crate::emit::narrow_long(&mut *c, value, 1); }
        if value.ty == 7 {
            let number = as_long(&mut *c, value);
            return crate::emit::narrow_long(&mut *c, number, 1);
        }
    }
    if word(&*c.source, method.start, method.end, &narrow_byte) {
        if value.ty == 4 { return value; }
        if value.ty == 1 { return crate::emit::narrow_byte(&mut *c, value, false); }
        if value.ty == 5 { return crate::emit::narrow_long(&mut *c, value, 4); }
        if value.ty == 7 {
            let number = as_long(&mut *c, value);
            return crate::emit::narrow_long(&mut *c, number, 4);
        }
    }
    return extended_conversion(&mut *c, value, method);
}
fn region_length(c: &mut Compiler, binding: Binding) -> Value {
    let len_name = b"len";
    expect(&mut *c, 43);
    let method = token(&*c, c.cursor);
    expect(&mut *c, 1); expect(&mut *c, 20); expect(&mut *c, 21);
    if !word(&*c.source, method.start, method.end, &len_name) { fail(&mut *c); return invalid(); }
    if is_slice(binding.ty) {
        let view = crate::emit::load(&mut *c, binding.slot, binding.ty);
        return crate::emit::extract_field(&mut *c, view, 1, 1);
    }
    let length = array_count(binding.ty);
    return crate::emit::constant(&mut *c, 1, length.to_i64());
}
fn enum_literal(c: &mut Compiler, index: i32, depth: i32) -> Value {
    if index < 0 { fail(&mut *c); return invalid(); }
    let row = c.records[index];
    if !row.tagged { fail(&mut *c); return invalid(); }
    expect(&mut *c, 45);
    let name = c.cursor;
    expect(&mut *c, 1);
    return enum_value(&mut *c, index, name, depth);
}
fn enum_value(c: &mut Compiler, index: i32, name: i32, depth: i32) -> Value {
    let row = c.records[index];
    let variant = record_field(&*c, row, name);
    if variant < 0 { fail(&mut *c); return invalid(); }
    let field = c.fields[row.first_field + variant];
    let tag = crate::emit::constant(&mut *c, 1, variant.to_i64());
    let tagged = crate::emit::insert_field(
        &mut *c, Value { ty: record_type(index), id: -1 }, tag, 0
    );
    if field.ty == 3 { return tagged; }
    expect(&mut *c, 20);
    let payload = expression(&mut *c, 1, depth + 1);
    expect(&mut *c, 21);
    if payload.ty != field.ty { fail(&mut *c); return invalid(); }
    return crate::emit::insert_field(&mut *c, tagged, payload, variant + 1);
}
fn record_literal(c: &mut Compiler, index: i32, depth: i32) -> Value {
    let row = c.records[index];
    let owner = c.functions[c.current];
    if row.tagged || index == c.byte_buffer { fail(&mut *c); return invalid(); }
    let mut seen = new::<bool>();
    while len(&seen) < row.field_count {
        if !grown(try_push(&mut seen, false)) { c.error = 68; return invalid(); }
    }
    expect(&mut *c, 22);
    let mut count = 0;
    let mut aggregate = Value { ty: record_type(index), id: -1 };
    while kind(&*c) != 23 && c.error == 0 {
        let field = visible_field(&*c, owner.source_module, row, c.cursor);
        expect(&mut *c, 1); expect(&mut *c, 24);
        if field < 0 || seen[field] { fail(&mut *c); return invalid(); }
        let descriptor = c.fields[row.first_field + field];
        let value = expression(&mut *c, 1, depth + 1);
        if value.ty != descriptor.ty { fail(&mut *c); return invalid(); }
        seen[field] = true;
        aggregate = crate::emit::insert_field(&mut *c, aggregate, value, field);
        count = count + 1;
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    if count != row.field_count { fail(&mut *c); return invalid(); }
    return aggregate;
}

fn field_path(c: &mut Compiler, binding: Binding) -> FieldPath {
    let owner = c.functions[c.current];
    let mut path = FieldPath { code: 1i64, depth: 0, ty: record_value_type(binding.ty), pointer: -1 };
    while kind(&*c) == 43 && c.error == 0 {
        if path.depth >= 8 { c.error = 68; return path; }
        if !is_record(path.ty) { fail(&mut *c); return path; }
        let row = c.records[record_index(path.ty)];
        if row.tagged { fail(&mut *c); return path; }
        expect(&mut *c, 43);
        let field = visible_field(&*c, owner.source_module, row, c.cursor);
        expect(&mut *c, 1);
        if field < 0 || c.error != 0 { fail(&mut *c); return path; }
        if path.depth == 0 {
            path.pointer = crate::emit::record_field_address(&mut *c, binding, field);
        } else {
            let parent = path.ty;
            let pointer = path.pointer;
            path.pointer = crate::emit::field_address(&mut *c, parent, pointer, false, field);
        }
        let descriptor = c.fields[row.first_field + field];
        path.ty = descriptor.ty;
        path.code = path.code * 64i64 + field.to_i64();
        path.depth = path.depth + 1;
    }
    return path;
}

fn field_value(c: &mut Compiler, binding: Binding, position: i32) -> Value {
    let path = field_path(&mut *c, binding);
    if c.error != 0 || !is_value_scalar(path.ty) || !access_allowed(&*c, position, path.code, path.depth, false) {
        fail(&mut *c); return invalid();
    }
    return crate::emit::load_element(&mut *c, path.pointer, path.ty);
}

fn moves_on_read(c: &Compiler, ty: i32) -> bool {
    if is_array(ty) { return true; }
    if is_record(ty) {
        let row = c.records[record_index(ty)];
        return !row.copy;
    }
    return false;
}
fn named_value(c: &mut Compiler, depth: i32) -> Value {
    let name = c.cursor;
    c.cursor = c.cursor + 1;
    let owner = c.functions[c.current];
    if kind(&*c) == 45 {
        let index = record_binding(&*c, owner.source_module, name);
        return enum_literal(&mut *c, index, depth);
    }
    if kind(&*c) == 20 {
        let target = function_binding(&*c, owner.source_module, name);
        return called(&mut *c, target, depth);
    }
    let position = local(&*c, name);
    if kind(&*c) == 22 {
        let separator = token(&*c, c.cursor + 2);
        if position < 0 || separator.kind == 24 {
        let index = record_binding(&*c, owner.source_module, name);
            if index >= 0 { return record_literal(&mut *c, index, depth); }
        }
    }
    if position < 0 { fail(&mut *c); return invalid(); }
    let binding = c.vars[position];
    if !binding.available { fail(&mut *c); return invalid(); }
    if (is_record(binding.ty) || is_record_reference(binding.ty)) && kind(&*c) == 43 {
        return field_value(&mut *c, binding, position);
    }
    if !access_allowed(&*c, position, 1i64, 0, false) { fail(&mut *c); return invalid(); }
    if kind(&*c) == 51 {
        if !is_array(binding.ty) && !is_slice(binding.ty) { fail(&mut *c); return invalid(); }
        expect(&mut *c, 51);
        let index = expression(&mut *c, 1, depth + 1);
        expect(&mut *c, 52);
        let target = c.vars[position];
        if index.ty != 1 || !target.available { fail(&mut *c); return invalid(); }
        if is_slice(binding.ty) { return crate::emit::slice_index(&mut *c, binding, index); }
        return crate::emit::array_index(&mut *c, binding, index);
    }
    if (is_array(binding.ty) || is_slice(binding.ty)) && kind(&*c) == 43 {
        return region_length(&mut *c, binding);
    }
    if is_slice(binding.ty) { fail(&mut *c); return invalid(); }
    if is_record_reference(binding.ty) { fail(&mut *c); return invalid(); }
    if moves_on_read(&*c, binding.ty) {
        // An outer binding must remain available for the next loop iteration.
        if position < c.loop_move_floor || !access_allowed(&*c, position, 1i64, 0, true) {
            fail(&mut *c); return invalid();
        }
        set_available(&mut *c, position, false);
    }
    let value = crate::emit::load(&mut *c, binding.slot, binding.ty);
    if needs_drop(&*c, binding.ty) {
        crate::emit::store(&mut *c, binding.slot, Value { ty: binding.ty, id: -1 });
    }
    if kind(&*c) == 43 { return convert_binding(&mut *c, value); }
    return value;
}
fn array_literal(c: &mut Compiler, depth: i32) -> Value {
    expect(&mut *c, 51);
    let mut element = 0;
    let mut values = new::<Value>();
    while kind(&*c) != 52 && c.error == 0 {
        if len(&values) >= 1024 { c.error = 68; return invalid(); }
        let value = expression(&mut *c, 1, depth + 1);
        if element == 0 { element = value.ty; }
        if !is_value_scalar(element) || value.ty != element { fail(&mut *c); return invalid(); }
        if !grown(try_push(&mut values, value)) { c.error = 68; return invalid(); }
        if kind(&*c) != 52 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 52);
    let count = len(&values);
    if count == 0 { fail(&mut *c); return invalid(); }
    let mut array = Value { ty: array_type(element, count), id: -1 };
    let mut index = 0;
    while index < count && c.error == 0 {
        array = crate::emit::insert_field(&mut *c, array, values[index], index);
        index = index + 1;
    }
    return array;
}
fn prefix(c: &mut Compiler, depth: i32) -> Value {
    if c.error != 0 { return invalid(); }
    if depth > 64 { c.error = 68; return invalid(); }
    let t = token(&*c, c.cursor);
    if t.kind == 30 {
        c.cursor = c.cursor + 1;
        let next = token(&*c, c.cursor);
        if next.kind == 2 || next.kind == 49 {
            c.cursor = c.cursor + 1;
            let mut ty = 1;
            if next.kind == 49 { ty = 5; }
            return crate::emit::constant(&mut *c, ty, next.value);
        }
        let negative_operand = prefix(&mut *c, depth + 1);
        if negative_operand.ty != 1 && negative_operand.ty != 5 { fail(&mut *c); return invalid(); }
        let zero = crate::emit::constant(&mut *c, negative_operand.ty, 0i64);
        return crate::emit::binary(&mut *c, 30, zero, negative_operand);
    }
    if t.kind == 34 {
        c.cursor = c.cursor + 1;
        let not_operand = prefix(&mut *c, depth + 1);
        if not_operand.ty != 2 { fail(&mut *c); return invalid(); }
        return crate::emit::negate_bool(&mut *c, not_operand);
    }
    if t.kind == 68 {
        c.cursor = c.cursor + 1;
        let inverted = prefix(&mut *c, depth + 1);
        if !integer_type(inverted.ty) { fail(&mut *c); return invalid(); }
        let all_bits = crate::emit::constant(&mut *c, inverted.ty, -1i64);
        return crate::emit::binary(&mut *c, 67, inverted, all_bits);
    }
    if t.kind == 2 || t.kind == 49 {
        let mut ty = 1;
        if t.kind == 49 { ty = 5; }
        if (ty == 1 && t.value == -2147483648i64) || t.value == -9223372036854775808i64 {
            fail(&mut *c); return invalid();
        }
        c.cursor = c.cursor + 1;
        return crate::emit::constant(&mut *c, ty, 0i64 - t.value);
    }
    if t.kind == 17 {
        c.cursor = c.cursor + 1;
        return crate::emit::constant(&mut *c, 4, t.value);
    }
    if t.kind == 59 {
        c.cursor = c.cursor + 1;
        return crate::emit::constant(&mut *c, 6, t.value);
    }
    if t.kind == 61 {
        c.cursor = c.cursor + 1;
        return crate::emit::constant(&mut *c, 7, t.value);
    }
    if t.kind == 50 {
        let index = c.cursor;
        c.cursor = c.cursor + 1;
        return crate::emit::byte_literal(index, t.decoded_length);
    }
    if t.kind == 12 || t.kind == 13 {
        c.cursor = c.cursor + 1;
        let mut boolean_literal = 0i64;
        if t.kind == 12 { boolean_literal = 1i64; }
        return crate::emit::constant(&mut *c, 2, boolean_literal);
    }
    if t.kind == 20 {
        c.cursor = c.cursor + 1;
        if kind(&*c) == 21 {
            c.cursor = c.cursor + 1;
            return Value { ty: 3, id: -1 };
        }
        let parenthesized = expression(&mut *c, 1, depth + 1);
        expect(&mut *c, 21);
        return parenthesized;
    }
    if t.kind == 51 { return array_literal(&mut *c, depth); }
    if t.kind == 44 { return qualified_value(&mut *c, depth); }
    if t.kind == 1 { return named_value(&mut *c, depth); }
    fail(&mut *c); return invalid();
}
fn short_circuit(c: &mut Compiler, op: i32, left: Value, rank: i32, depth: i32) -> Value {
    if left.ty != 2 { fail(&mut *c); return invalid(); }
    let slot = crate::emit::slot(&mut *c, 2);
    let rhs = crate::emit::new_label(&mut *c);
    let shortcut = crate::emit::new_label(&mut *c);
    let merge = crate::emit::new_label(&mut *c);
    if op == 41 { crate::emit::conditional(&mut *c, left, rhs, shortcut); }
    else { crate::emit::conditional(&mut *c, left, shortcut, rhs); }
    crate::emit::label(&mut *c, shortcut);
    let mut bit = 0i64;
    if op == 42 { bit = 1i64; }
    let value = crate::emit::constant(&mut *c, 2, bit);
    crate::emit::store(&mut *c, slot, value);
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, rhs);
    let right = expression(&mut *c, rank + 1, depth + 1);
    if right.ty != 2 { fail(&mut *c); return invalid(); }
    crate::emit::store(&mut *c, slot, right);
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, merge);
    return crate::emit::load(&mut *c, slot, 2);
}
pub fn expression(c: &mut Compiler, minimum: i32, depth: i32) -> Value {
    if depth > 64 { c.error = 68; return invalid(); }
    let mut left = prefix(&mut *c, depth);
    while c.error == 0 {
        let op = infix(&*c);
        let rank = precedence(op);
        if rank < minimum || rank == 0 { return left; }
        c.cursor = c.cursor + 1;
        if op == 41 || op == 42 {
            left = short_circuit(&mut *c, op, left, rank, depth);
            continue;
        }
        let shifting = op == 69 || op == 70;
        if shifting { c.cursor = c.cursor + 1; }
        let right = expression(&mut *c, rank + 1, depth + 1);
        if shifting {
            if !integer_type(left.ty) || !integer_type(right.ty) { fail(&mut *c); return invalid(); }
            left = crate::emit::shift(&mut *c, op, left, right);
            continue;
        }
        if left.ty != right.ty || left.ty == 3 || left.ty == 0
            || is_array(left.ty) || is_record(left.ty) { fail(&mut *c); return invalid(); }
        if left.ty == 6 && ((op >= 29 && op <= 33) || (op >= 65 && op <= 67)) {
            fail(&mut *c); return invalid();
        }
        if op != 35 && op != 36 && !integer_type(left.ty) && left.ty != 6 {
            fail(&mut *c); return invalid();
        }
        left = crate::emit::binary(&mut *c, op, left, right);
    }
    return invalid();
}
fn destructure(c: &mut Compiler) -> () {
    let owner = c.functions[c.current];
    let index = record_binding(&*c, owner.source_module, c.cursor);
    if index < 0 { fail(&mut *c); return; }
    let row = c.records[index];
    if row.tagged || index == c.byte_buffer { fail(&mut *c); return; }
    expect(&mut *c, 1);
    expect(&mut *c, 22);
    let mut destinations = new::<i32>();
    while len(&destinations) < row.field_count {
        if !grown(try_push(&mut destinations, -1)) { c.error = 68; return; }
    }
    let mut count = 0;
    while kind(&*c) != 23 && c.error == 0 {
        let field_name = c.cursor;
        let field = visible_field(&*c, owner.source_module, row, field_name);
        expect(&mut *c, 1);
        if field < 0 || destinations[field] >= 0 { fail(&mut *c); return; }
        expect(&mut *c, 24);
        let name = c.cursor;
        expect(&mut *c, 1);
        destinations[field] = name;
        count = count + 1;
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    expect(&mut *c, 28);
    if count != row.field_count { fail(&mut *c); return; }
    let aggregate = expression(&mut *c, 1, 0);
    expect(&mut *c, 25);
    if c.error != 0 || aggregate.ty != record_type(index) { fail(&mut *c); return; }
    for field in 0..row.field_count {
        let descriptor = c.fields[row.first_field + field];
        let value = crate::emit::extract_field(&mut *c, aggregate, field, descriptor.ty);
        let slot = crate::emit::slot(&mut *c, value.ty);
        bind(&mut *c, destinations[field], value.ty, false, slot);
        crate::emit::store(&mut *c, slot, value);
    }
    return;
}
fn declared(c: &mut Compiler) -> () {
    expect(&mut *c, 4);
    let next = token(&*c, c.cursor + 1);
    if kind(&*c) == 1 && next.kind == 22 { destructure(&mut *c); return; }
    let mut writable = false;
    if kind(&*c) == 5 { writable = true; c.cursor = c.cursor + 1; }
    let name = c.cursor;
    expect(&mut *c, 1);
    let mut expected = 0;
    if kind(&*c) == 24 {
        c.cursor = c.cursor + 1;
        let owner = c.functions[c.current];
        expected = parse_type(&mut *c, owner.source_module);
    }
    expect(&mut *c, 28);
    let initializer = c.cursor;
    let value = expression(&mut *c, 1, 0);
    if expected != 0 && value.ty != expected {
        if c.error == 0 { c.cursor = initializer; }
        fail(&mut *c); return;
    }
    expect(&mut *c, 25);
    let slot = crate::emit::slot(&mut *c, value.ty);
    bind(&mut *c, name, value.ty, writable, slot);
    crate::emit::store(&mut *c, slot, value);
    return;
}
fn assigned(c: &mut Compiler) -> () {
    let name = c.cursor;
    let position = local(&*c, name);
    if position < 0 { fail(&mut *c); return; }
    let binding = c.vars[position];
    if !binding.writable { fail(&mut *c); return; }
    c.cursor = c.cursor + 2;
    let initializer = c.cursor;
    let value = expression(&mut *c, 1, 0);
    if value.ty != binding.ty {
        if c.error == 0 { c.cursor = initializer; }
        fail(&mut *c); return;
    }
    expect(&mut *c, 25);
    crate::emit::drop_owner(&mut *c, binding.ty, binding.slot);
    set_available(&mut *c, position, true);
    crate::emit::store(&mut *c, binding.slot, value);
    return;
}
fn assign_field(c: &mut Compiler) -> () {
    let position = local(&*c, c.cursor);
    if position < 0 { fail(&mut *c); return; }
    let binding = c.vars[position];
    let borrowed = is_mutable_record_reference(binding.ty);
    if !borrowed && (!binding.writable || !is_record(binding.ty)) { fail(&mut *c); return; }
    c.cursor = c.cursor + 1;
    let path = field_path(&mut *c, binding);
    expect(&mut *c, 28);
    if c.error != 0 || !is_value_scalar(path.ty) { fail(&mut *c); return; }
    let value = expression(&mut *c, 1, 0);
    if value.ty != path.ty { fail(&mut *c); return; }
    expect(&mut *c, 25);
    let target = c.vars[position];
    if !target.available || !access_allowed(&*c, position, path.code, path.depth, true) { fail(&mut *c); return; }
    crate::emit::store_element(&mut *c, path.pointer, value);
    return;
}
fn field_assignment(c: &Compiler) -> bool {
    let mut index = c.cursor + 1;
    let mut depth = 0;
    while depth < 8 {
        let mark = token(&*c, index);
        if mark.kind != 43 { return mark.kind == 28; }
        let name = token(&*c, index + 1);
        if name.kind != 1 { return false; }
        index = index + 2;
        depth = depth + 1;
    }
    let mark = token(&*c, index);
    return mark.kind == 28;
}
fn index_assignment_end(c: &Compiler, open: i32) -> i32 {
    let mut at = open + 1;
    let mut depth = 1;
    while depth > 0 {
        let current = token(&*c, at);
        if current.kind == 0 || current.kind == 23 || current.kind == 25 { return -1; }
        if current.kind == 51 { depth = depth + 1; }
        if current.kind == 52 { depth = depth - 1; }
        at = at + 1;
    }
    let following = token(&*c, at);
    if following.kind == 28 { return at - 1; }
    return -1;
}
fn index_assignment(c: &mut Compiler, end: i32) -> () {
    let position = local(&*c, c.cursor);
    if position < 0 { fail(&mut *c); return; }
    let binding = c.vars[position];
    let view = is_mutable_slice(binding.ty);
    let mut element = 0;
    if view { element = slice_element(binding.ty); }
    else {
        if !binding.writable || !is_array(binding.ty) { fail(&mut *c); return; }
        element = array_element(binding.ty);
    }
    let begin = c.cursor + 2;
    // Like other Oxid array assignments, evaluate the right side before the index.
    c.cursor = end + 2;
    let value = expression(&mut *c, 1, 0);
    if value.ty != element { fail(&mut *c); return; }
    expect(&mut *c, 25);
    if c.error != 0 { return; }
    let continuation = c.cursor;
    c.cursor = begin;
    let index = expression(&mut *c, 1, 0);
    if index.ty != 1 || c.cursor != end { fail(&mut *c); return; }
    c.cursor = continuation;
    let target = c.vars[position];
    if !target.available || !access_allowed(&*c, position, 1i64, 0, true) { fail(&mut *c); return; }
    if view { crate::emit::slice_store(&mut *c, binding, index, value); }
    else { crate::emit::array_store(&mut *c, binding, index, value); }
    return;
}
fn if_statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    if depth > 64 { c.error = 68; return; }
    expect(&mut *c, 7);
    let before = c.live;
    let condition = expression(&mut *c, 1, 0);
    if condition.ty != 2 { fail(&mut *c); return; }
    let initial = availability(&mut *c);
    let yes = crate::emit::new_label(&mut *c);
    let no = crate::emit::new_label(&mut *c);
    let merge = crate::emit::new_label(&mut *c);
    crate::emit::conditional(&mut *c, condition, yes, no);
    crate::emit::label(&mut *c, yes); c.live = before;
    block(&mut *c, break_to, continue_to, depth + 1);
    let left_falls = c.live;
    let left = availability(&mut *c);
    restore_availability(&mut *c, &initial);
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, no); c.live = before;
    if kind(&*c) == 8 {
        c.cursor = c.cursor + 1;
        if kind(&*c) == 7 { if_statement(&mut *c, break_to, continue_to, depth + 1); }
        else { block(&mut *c, break_to, continue_to, depth + 1); }
    }
    let right_falls = c.live;
    if left_falls && c.error == 0 {
        if right_falls { intersect_availability(&mut *c, &left); }
        else { restore_availability(&mut *c, &left); }
    }
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, merge);
    c.live = left_falls || right_falls;
    return;
}
fn match_variant(c: &mut Compiler, row: Record, type_index: i32) -> i32 {
    let owner = c.functions[c.current];
    let mut arm_type = -1;
    let mut name = c.cursor;
    if kind(&*c) == 44 {
        let first = c.cursor + 2;
        name = path_name(&mut *c);
        arm_type = qualified_enum(&*c, first, name, owner.source_module);
    } else {
        arm_type = record_binding(&*c, owner.source_module, c.cursor);
        expect(&mut *c, 1); expect(&mut *c, 45);
        name = c.cursor;
        expect(&mut *c, 1);
    }
    let variant = record_field(&*c, row, name);
    if arm_type != type_index || variant < 0 { fail(&mut *c); return -1; }
    return variant;
}
fn match_payload(c: &mut Compiler, aggregate: Value, variant: i32, ty: i32) -> () {
    if ty == 3 { return; }
    expect(&mut *c, 20);
    let binding = c.cursor;
    expect(&mut *c, 1);
    expect(&mut *c, 21);
    if c.error != 0 { return; }
    let name = token(&*c, binding);
    let discard = b"_";
    if word(&*c.source, name.start, name.end, &discard) { fail(&mut *c); return; }
    let payload = crate::emit::extract_field(&mut *c, aggregate, variant + 1, ty);
    let slot = crate::emit::slot(&mut *c, ty);
    bind(&mut *c, binding, ty, false, slot);
    crate::emit::store(&mut *c, slot, payload);
    return;
}
fn match_statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    let unreachable = b"  unreachable\n";
    expect(&mut *c, 63);
    let following = token(&*c, c.cursor + 1);
    if kind(&*c) != 1 || following.kind != 22 || local(&*c, c.cursor) < 0 { fail(&mut *c); return; }
    let was_live = c.live;
    let aggregate = named_value(&mut *c, 0);
    if !is_record(aggregate.ty) { fail(&mut *c); return; }
    let type_index = record_index(aggregate.ty);
    let row = c.records[type_index];
    if !row.tagged { fail(&mut *c); return; }
    expect(&mut *c, 22);

    let mut labels = new::<i32>();
    let mut seen = new::<bool>();
    for variant in 0..row.field_count {
        let label = crate::emit::new_label(&mut *c);
        if !grown(try_push(&mut labels, label)) || !grown(try_push(&mut seen, false)) {
            c.error = 68; return;
        }
    }
    let tag = crate::emit::extract_field(&mut *c, aggregate, 0, 1);
    let bad_tag = crate::emit::new_label(&mut *c);
    let merge = crate::emit::new_label(&mut *c);
    crate::emit::enum_switch(&mut *c, tag, &labels, bad_tag);

    let incoming = availability(&mut *c);
    let locals = len(&incoming);
    let mut outgoing = new::<bool>();
    let mut falls_through = false;
    let mut count = 0;
    while kind(&*c) != 23 && c.error == 0 {
        let variant = match_variant(&mut *c, row, type_index);
        if variant < 0 || seen[variant] { fail(&mut *c); return; }
        seen[variant] = true;
        count = count + 1;
        restore_availability(&mut *c, &incoming);
        crate::emit::label(&mut *c, labels[variant]);
        c.live = was_live;
        let payload = c.fields[row.first_field + variant];
        match_payload(&mut *c, aggregate, variant, payload.ty);
        expect(&mut *c, 64);
        block(&mut *c, break_to, continue_to, depth + 1);
        drop_locals(&mut *c, locals);
        truncate(&mut *c.vars, locals);
        if c.live && c.error == 0 {
            if falls_through { intersect_availability(&mut *c, &outgoing); }
            outgoing = availability(&mut *c);
            falls_through = true;
        }
        crate::emit::branch(&mut *c, merge);
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    if count != row.field_count { fail(&mut *c); return; }
    crate::emit::label(&mut *c, bad_tag);
    crate::emit::append(&mut *c, 2, &unreachable);
    crate::emit::label(&mut *c, merge);
    if falls_through { restore_availability(&mut *c, &outgoing); }
    c.live = falls_through;
    return;
}
fn while_statement(c: &mut Compiler, depth: i32) -> () {
    expect(&mut *c, 9);
    let before = c.live;
    let initial = availability(&mut *c);
    let previous_floor = c.loop_move_floor;
    c.loop_move_floor = len(&*c.vars);
    let condition_label = crate::emit::new_label(&mut *c);
    let body_label = crate::emit::new_label(&mut *c);
    let end = crate::emit::new_label(&mut *c);
    crate::emit::branch(&mut *c, condition_label);
    crate::emit::label(&mut *c, condition_label);
    let condition = expression(&mut *c, 1, 0);
    if condition.ty != 2 { fail(&mut *c); return; }
    crate::emit::conditional(&mut *c, condition, body_label, end);
    crate::emit::label(&mut *c, body_label); c.live = before;
    block(&mut *c, end, condition_label, depth + 1);
    crate::emit::branch(&mut *c, condition_label);
    crate::emit::label(&mut *c, end); c.live = before;
    restore_availability(&mut *c, &initial);
    c.loop_move_floor = previous_floor;
    return;
}
fn for_statement(c: &mut Compiler, depth: i32) -> () {
    expect(&mut *c, 55);
    let name = c.cursor;
    expect(&mut *c, 1); expect(&mut *c, 56);
    let first = expression(&mut *c, 1, 0);
    expect(&mut *c, 57);
    let limit = expression(&mut *c, 1, 0);
    let ty = first.ty;
    if ty != limit.ty || !integer_type(ty) { fail(&mut *c); return; }

    let before = c.live;
    let slot = crate::emit::slot(&mut *c, ty);
    crate::emit::store(&mut *c, slot, first);
    let test = crate::emit::new_label(&mut *c);
    let body = crate::emit::new_label(&mut *c);
    let advance = crate::emit::new_label(&mut *c);
    let end = crate::emit::new_label(&mut *c);
    crate::emit::branch(&mut *c, test);
    crate::emit::label(&mut *c, test);
    let index = crate::emit::load(&mut *c, slot, ty);
    let has_next = crate::emit::binary(&mut *c, 37, index, limit);
    crate::emit::conditional(&mut *c, has_next, body, end);

    crate::emit::label(&mut *c, body); c.live = before;
    let initial = availability(&mut *c);
    let saved = len(&*c.vars);
    let previous_floor = c.loop_move_floor;
    c.loop_move_floor = saved;
    bind(&mut *c, name, ty, false, slot);
    block(&mut *c, end, advance, depth + 1);
    truncate(&mut *c.vars, saved);
    crate::emit::branch(&mut *c, advance);

    crate::emit::label(&mut *c, advance);
    let one = crate::emit::constant(&mut *c, ty, 1i64);
    let next = crate::emit::binary(&mut *c, 29, index, one);
    crate::emit::store(&mut *c, slot, next);
    crate::emit::branch(&mut *c, test);
    crate::emit::label(&mut *c, end); c.live = before;
    restore_availability(&mut *c, &initial);
    c.loop_move_floor = previous_floor;
    return;
}
fn drop_locals(c: &mut Compiler, floor: i32) -> () {
    if c.byte_buffer < 0 || !c.live { return; }
    let mut index = len(&*c.vars);
    while index > floor {
        index = index - 1;
        let binding = c.vars[index];
        // A moved slot holds zeroed fields even at a branch join where
        // the same slot can still own an allocation on the other path.
        crate::emit::drop_owner(&mut *c, binding.ty, binding.slot);
    }
    return;
}

fn statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    crate::emit::ensure(&mut *c);
    let k = kind(&*c);
    if k == 4 { declared(&mut *c); return; }
    if k == 7 { if_statement(&mut *c, break_to, continue_to, depth); return; }
    if k == 63 { match_statement(&mut *c, break_to, continue_to, depth); return; }
    if k == 9 { while_statement(&mut *c, depth); return; }
    if k == 55 { for_statement(&mut *c, depth); return; }
    if k == 22 { block(&mut *c, break_to, continue_to, depth + 1); return; }
    if k == 10 || k == 11 {
        if break_to < 0 || continue_to < 0 { fail(&mut *c); return; }
        c.cursor = c.cursor + 1; expect(&mut *c, 25);
        let floor = c.loop_move_floor;
        drop_locals(&mut *c, floor);
        if k == 10 { crate::emit::branch(&mut *c, break_to); }
        else { crate::emit::branch(&mut *c, continue_to); }
        c.live = false; return;
    }
    if k == 6 {
        c.cursor = c.cursor + 1;
        let value_start = c.cursor;
        let mut value = Value { ty: 3, id: -1 };
        if kind(&*c) != 25 { value = expression(&mut *c, 1, 0); }
        expect(&mut *c, 25);
        let f = c.functions[c.current];
        if value.ty != f.result {
            if c.error == 0 { c.cursor = value_start; }
            fail(&mut *c); return;
        }
        drop_locals(&mut *c, 0);
        crate::emit::returned(&mut *c, value); return;
    }
    let next = token(&*c, c.cursor + 1);
    if k == 1 && next.kind == 28 { assigned(&mut *c); return; }
    if k == 1 && next.kind == 43 && field_assignment(&*c) { assign_field(&mut *c); return; }
    if k == 1 && next.kind == 51 {
        let end = index_assignment_end(&*c, c.cursor + 1);
        if end >= 0 { index_assignment(&mut *c, end); return; }
    }
    let value = expression(&mut *c, 1, 0);
    expect(&mut *c, 25);
    if needs_drop(&*c, value.ty) {
        let temporary = crate::emit::slot(&mut *c, value.ty);
        crate::emit::store(&mut *c, temporary, value);
        crate::emit::drop_owner(&mut *c, value.ty, temporary);
    }
    return;
}
pub fn block(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    if depth > 64 { c.error = 68; return; }
    expect(&mut *c, 22);
    let saved = len(&*c.vars);
    while kind(&*c) != 23 && c.error == 0 {
        if kind(&*c) == 0 { fail(&mut *c); return; }
        statement(&mut *c, break_to, continue_to, depth);
    }
    expect(&mut *c, 23);
    drop_locals(&mut *c, saved);
    truncate(&mut *c.vars, saved);
    return;
}
pub fn bodies(c: &mut Compiler) -> () {
    let literal_bytes_0 = b"  store ";
    let literal_bytes_1 = b" %a";
    let literal_bytes_2 = b", ptr %s";
    let literal_bytes_3 = b"\n";
    let literal_bytes_4 = b"  unreachable\n";

    let mut i = 0;
    while i < len(&*c.functions) && c.error == 0 {
        let f = c.functions[i];
        c.error_module = f.source_module;
        c.current = i; c.cursor = f.begin; c.next_value = 0; c.next_label = 0;
        c.next_slot = 0; c.loop_move_floor = 0; c.live = true; c.terminated = false;
        truncate(&mut *c.vars, 0);
        let mut p = 0;
        while p < f.count && c.error == 0 {
            let parameter = c.params[f.params + p];
            let slot = crate::emit::slot(&mut *c, parameter.ty);
            bind(&mut *c, parameter.name, parameter.ty, false, slot);
            if parameter.ty != 3 {
                crate::emit::append(&mut *c, 2, &literal_bytes_0); crate::emit::write_type(&mut *c, 2, parameter.ty, false);
                crate::emit::append(&mut *c, 2, &literal_bytes_1); crate::emit::number(&mut *c, 2, p);
                crate::emit::append(&mut *c, 2, &literal_bytes_2); crate::emit::number(&mut *c, 2, slot);
                crate::emit::append(&mut *c, 2, &literal_bytes_3);
            }
            p = p + 1;
        }
        block(&mut *c, -1, -1, 0);
        if c.cursor != f.end { fail(&mut *c); return; }
        if c.live {
            if f.result != 3 { fail(&mut *c); return; }
            drop_locals(&mut *c, 0);
            crate::emit::returned(&mut *c, Value { ty: 3, id: -1 });
        } else {
            if !c.terminated { crate::emit::append(&mut *c, 2, &literal_bytes_4); c.terminated = true; }
        }
        crate::emit::finish_function(&mut *c, i);
        i = i + 1;
    }
    return;
}
