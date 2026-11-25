use crate::model::Compiler;
use crate::model::Value;
use crate::model::grown;
use crate::model::Binding;
use crate::model::Record;
use crate::model::Token;
use crate::model::token;
use crate::model::array_type;
use crate::model::is_array;
use crate::model::array_count;
use crate::model::array_element;
use crate::model::is_slice;
use crate::model::slice_element;
use crate::model::is_record_reference;
use crate::model::record_value_type;
use crate::model::record_type;
use crate::model::value_cells;
use crate::model::read_some_binding;
use crate::model::code;
use crate::model::is_record;
use crate::model::record_index;
use crate::model::is_byte_buffer;
use crate::model::is_file;
use crate::model::needs_drop;
use crate::model::uses_heap;
use crate::model::vector_element;
use crate::model::vector_runtime_tag;
use crate::model::referent_record;
use crate::model::copy_record_shape;
use alloc::vec::len as vector_len;
use alloc::vec::Vec;
use alloc::bytes::len;
use alloc::bytes::try_extend;
use alloc::bytes::take;

pub fn append(c: &mut Compiler, destination: i32, bytes: &[u8]) -> () {
    if c.error != 0 { return; }
    let mut size = len(&*c.body);
    if destination == 0 { size = len(&*c.module); }
    if destination == 1 { size = len(&*c.slots); }
    if bytes.len() > 8388608 - size { c.error = 68; return; }
    let mut ok = false;
    if destination == 0 { ok = grown(try_extend(&mut *c.module, &*bytes)); }
    else {
        if destination == 1 { ok = grown(try_extend(&mut *c.slots, &*bytes)); }
        else { ok = grown(try_extend(&mut *c.body, &*bytes)); }
    }
    if !ok { c.error = 68; }
    return;
}
pub fn number(c: &mut Compiler, destination: i32, value: i32) -> () {
    decimal(&mut *c, destination, value.to_i64());
    return;
}
fn decimal(c: &mut Compiler, destination: i32, value: i64) -> () {
    let minus = b"-";
    let mut digits = b"00000000000000000000";
    let mut at = 20;
    let mut remaining = value;
    if remaining > 0i64 { remaining = 0i64 - remaining; }
    while remaining != 0i64 || at == 20 {
        at = at - 1;
        let digit_code = 48i64 - remaining % 10i64;
        digits[at] = digit_code.to_u8_checked();
        remaining = remaining / 10i64;
    }
    if value < 0i64 { append(&mut *c, destination, &minus); }
    append(&mut *c, destination, &digits[at..20]);
    return;
}
fn write_scalar_type(c: &mut Compiler, destination: i32, ty: i32, result: bool) -> () {
    let int_type = b"i32";
    let unit_result = b"void";
    let bool_type = b"i1";
    let byte_type = b"i8";
    let long_type = b"i64";
    if ty == 1 || ty == 6 || ty == 7 { append(&mut *c, destination, &int_type); return; }
    if ty == 4 { append(&mut *c, destination, &byte_type); return; }
    if ty == 5 { append(&mut *c, destination, &long_type); return; }
    if ty == 3 && result { append(&mut *c, destination, &unit_result); }
    else { append(&mut *c, destination, &bool_type); }
    return;
}
pub fn write_type(c: &mut Compiler, destination: i32, ty: i32, result: bool) -> () {
    if is_record_reference(ty) {
        let pointer = b"ptr";
        append(&mut *c, destination, &pointer);
        return;
    }
    if is_slice(ty) {
        let view = b"{ ptr, i32 }";
        append(&mut *c, destination, &view);
        return;
    }
    if is_record(ty) {
        let record = b"%r";
        append(&mut *c, destination, &record);
        number(&mut *c, destination, record_index(ty));
        return;
    }
    if is_array(ty) {
        let open = b"[";
        let separator = b" x ";
        let end = b"]";
        append(&mut *c, destination, &open);
        number(&mut *c, destination, array_count(ty));
        append(&mut *c, destination, &separator);
        write_scalar_type(&mut *c, destination, array_element(ty), false);
        append(&mut *c, destination, &end);
        return;
    }
    write_scalar_type(&mut *c, destination, ty, result);
    return;
}
pub fn operand(c: &mut Compiler, destination: i32, value: Value) -> () {
    let literal_bytes_0 = b"false";
    let literal_bytes_1 = b"%v";
    let empty_record = b"zeroinitializer";

    if value.ty == 3 { append(&mut *c, destination, &literal_bytes_0); }
    else if is_record(value.ty) && value.id == -1 { append(&mut *c, destination, &empty_record); }
    else if value.id < -1 {
        let row = token(&*c, -2 - value.id);
        literal_text(&mut *c, destination, row);
    }
    else {
        append(&mut *c, destination, &literal_bytes_1);
        number(&mut *c, destination, value.id);
    }
    return;
}
pub fn fresh(c: &mut Compiler) -> i32 {
    let value = c.next_value;
    if value >= 131072 { c.error = 68; return 0; }
    c.next_value = value + 1;
    return value;
}
pub fn new_label(c: &mut Compiler) -> i32 {
    let value = c.next_label;
    if value >= 65536 { c.error = 68; return 0; }
    c.next_label = value + 1;
    return value;
}
pub fn label(c: &mut Compiler, id: i32) -> () {
    let literal_bytes_0 = b"b";
    let literal_bytes_1 = b":\n";

    append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, id); append(&mut *c, 2, &literal_bytes_1);
    c.terminated = false;
    return;
}
pub fn ensure(c: &mut Compiler) -> () {
    if c.terminated { let id = new_label(&mut *c); label(&mut *c, id); }
    return;
}
pub fn branch(c: &mut Compiler, target: i32) -> () {
    let literal_bytes_0 = b"  br label %b";
    let literal_bytes_1 = b"\n";

    if !c.terminated {
        append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, target); append(&mut *c, 2, &literal_bytes_1);
        c.terminated = true;
    }
    return;
}
pub fn conditional(c: &mut Compiler, value: Value, yes: i32, no: i32) -> () {
    let literal_bytes_0 = b"  br i1 ";
    let literal_bytes_1 = b", label %b";
    let literal_bytes_2 = b"\n";

    append(&mut *c, 2, &literal_bytes_0); operand(&mut *c, 2, value);
    append(&mut *c, 2, &literal_bytes_1); number(&mut *c, 2, yes);
    append(&mut *c, 2, &literal_bytes_1); number(&mut *c, 2, no); append(&mut *c, 2, &literal_bytes_2);
    c.terminated = true;
    return;
}
pub fn slot(c: &mut Compiler, ty: i32) -> i32 {
    let literal_bytes_0 = b"  %s";
    let literal_bytes_1 = b" = alloca ";
    let literal_bytes_2 = b"\n";

    if ty == 3 { return -1; }
    let id = c.next_slot;
    let cells = value_cells(&*c, ty);
    if cells > 4096 - id { c.error = 68; return 0; }
    c.next_slot = id + cells;
    append(&mut *c, 1, &literal_bytes_0); number(&mut *c, 1, id);
    append(&mut *c, 1, &literal_bytes_1); write_type(&mut *c, 1, ty, false);
    append(&mut *c, 1, &literal_bytes_2);
    return id;
}
pub fn store(c: &mut Compiler, position: i32, value: Value) -> () {
    if value.id < -1 {
        store_byte_literal(&mut *c, position, value);
        return;
    }
    let literal_bytes_0 = b"  store ";
    let literal_bytes_1 = b" ";
    let literal_bytes_2 = b", ptr %s";
    let literal_bytes_3 = b"\n";

    if value.ty == 3 { return; }
    append(&mut *c, 2, &literal_bytes_0); write_type(&mut *c, 2, value.ty, false);
    append(&mut *c, 2, &literal_bytes_1); operand(&mut *c, 2, value);
    append(&mut *c, 2, &literal_bytes_2); number(&mut *c, 2, position); append(&mut *c, 2, &literal_bytes_3);
    return;
}
pub fn load(c: &mut Compiler, position: i32, ty: i32) -> Value {
    let literal_bytes_0 = b"  %v";
    let literal_bytes_1 = b" = load ";
    let literal_bytes_2 = b", ptr %s";
    let literal_bytes_3 = b"\n";

    if ty == 3 { return Value { ty: 3, id: -1 }; }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, id);
    append(&mut *c, 2, &literal_bytes_1); write_type(&mut *c, 2, ty, false);
    append(&mut *c, 2, &literal_bytes_2); number(&mut *c, 2, position); append(&mut *c, 2, &literal_bytes_3);
    return Value { ty: ty, id: id };
}
pub fn constant(c: &mut Compiler, ty: i32, value: i64) -> Value {
    let prefix = b"  %v";
    let numeric = b" = add ";
    let zero = b" 0, ";
    let boolean = b" = icmp ne i32 0, ";
    let line = b"\n";

    if ty == 3 { return Value { ty: 3, id: -1 }; }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    if ty == 2 { append(&mut *c, 2, &boolean); }
    else {
        append(&mut *c, 2, &numeric);
        write_type(&mut *c, 2, ty, false);
        append(&mut *c, 2, &zero);
    }
    decimal(&mut *c, 2, value); append(&mut *c, 2, &line);
    return Value { ty: ty, id: id };
}
fn literal_name(c: &mut Compiler, destination: i32, index: i32) -> () {
    let prefix = b"@data_";
    append(&mut *c, destination, &prefix);
    number(&mut *c, destination, index);
    return;
}
fn hex_byte(value: i32) -> u8 {
    let mut ascii = value + 48;
    if value > 9 { ascii = value + 55; }
    return ascii.to_u8_checked();
}
fn literal_text(c: &mut Compiler, destination: i32, row: Token) -> () {
    let data = b"c\"";
    let end = b"\"";
    append(&mut *c, destination, &data);
    let mut at = row.decoded_start;
    while at < row.decoded_start + row.decoded_length && c.error == 0 {
        let value = code(&*c.literals, at);
        let escaped = [92u8, hex_byte(value / 16), hex_byte(value % 16)];
        append(&mut *c, destination, &escaped);
        at = at + 1;
    }
    append(&mut *c, destination, &end);
    return;
}
fn literal_constant(c: &mut Compiler, index: i32, row: Token) -> () {
    let prefix = b" = private constant ";
    let space = b" ";
    let line = b"\n";
    literal_name(&mut *c, 0, index);
    append(&mut *c, 0, &prefix);
    write_type(&mut *c, 0, array_type(4, row.decoded_length), false);
    append(&mut *c, 0, &space); literal_text(&mut *c, 0, row); append(&mut *c, 0, &line);
    return;
}
fn byte_literals(c: &mut Compiler) -> () {
    let intrinsic = b"declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1 immarg)\n";
    append(&mut *c, 0, &intrinsic);
    let mut index = 0;
    while index < vector_len(&*c.tokens) && c.error == 0 {
        let row = c.tokens[index];
        if row.kind == 50 { literal_constant(&mut *c, index, row); }
        index = index + 1;
    }
    return;
}
pub fn byte_literal(index: i32, length: i32) -> Value {
    // Leave large byte constants out of SSA so a local store can use one copy.
    return Value { ty: array_type(4, length), id: -2 - index };
}
fn store_byte_literal(c: &mut Compiler, position: i32, value: Value) -> () {
    let copy = b"  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %s";
    let pointer = b", ptr align 1 ";
    let width = b", i64 ";
    let end = b", i1 false)\n";
    append(&mut *c, 2, &copy); number(&mut *c, 2, position); append(&mut *c, 2, &pointer);
    literal_name(&mut *c, 2, -2 - value.id);
    append(&mut *c, 2, &width); number(&mut *c, 2, array_count(value.ty));
    append(&mut *c, 2, &end);
    return;
}
pub fn insert_field(c: &mut Compiler, aggregate: Value, value: Value, field: i32) -> Value {
    let name = b"  %v";
    let insert = b" = insertvalue ";
    let first = b" zeroinitializer";
    let space = b" ";
    let comma = b", ";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &insert);
    write_type(&mut *c, 2, aggregate.ty, false);
    if aggregate.id < 0 { append(&mut *c, 2, &first); }
    else { append(&mut *c, 2, &space); operand(&mut *c, 2, aggregate); }
    append(&mut *c, 2, &comma); write_type(&mut *c, 2, value.ty, false);
    append(&mut *c, 2, &space); operand(&mut *c, 2, value);
    append(&mut *c, 2, &comma); number(&mut *c, 2, field); append(&mut *c, 2, &line);
    return Value { ty: aggregate.ty, id: id };
}
pub fn extract_field(c: &mut Compiler, aggregate: Value, field: i32, ty: i32) -> Value {
    let name = b"  %v";
    let extract = b" = extractvalue ";
    let space = b" ";
    let comma = b", ";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &extract);
    write_type(&mut *c, 2, aggregate.ty, false);
    append(&mut *c, 2, &space); operand(&mut *c, 2, aggregate);
    append(&mut *c, 2, &comma); number(&mut *c, 2, field); append(&mut *c, 2, &line);
    return Value { ty: ty, id: id };
}
fn require_index_bounds(c: &mut Compiler, within: Value) -> () {
    let failure = b"  call void @__oxid_overflow(ptr @index_error, i64 26)\n  unreachable\n";
    let valid = new_label(&mut *c);
    let invalid = new_label(&mut *c);
    conditional(&mut *c, within, valid, invalid);
    label(&mut *c, invalid);
    append(&mut *c, 2, &failure);
    label(&mut *c, valid);
    return;
}
fn check_index(c: &mut Compiler, index: Value, length: Value) -> () {
    let name = b"  %v";
    let compare = b" = icmp ult i32 ";
    let comma = b", ";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &compare);
    operand(&mut *c, 2, index); append(&mut *c, 2, &comma);
    operand(&mut *c, 2, length); append(&mut *c, 2, &line);
    require_index_bounds(&mut *c, Value { ty: 2, id: id });
    return;
}
fn array_address(c: &mut Compiler, binding: Binding, index: Value) -> i32 {
    let name = b"  %v";
    let element = b" = getelementptr inbounds ";
    let slot_prefix = b", ptr %s";
    let ordinal = b", i32 0, i32 ";
    let line = b"\n";
    let count = array_count(binding.ty);
    let length = constant(&mut *c, 1, count.to_i64());
    check_index(&mut *c, index, length);
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, pointer); append(&mut *c, 2, &element);
    write_type(&mut *c, 2, binding.ty, false); append(&mut *c, 2, &slot_prefix);
    number(&mut *c, 2, binding.slot); append(&mut *c, 2, &ordinal);
    operand(&mut *c, 2, index); append(&mut *c, 2, &line);
    return pointer;
}
pub fn load_element(c: &mut Compiler, pointer: i32, element: i32) -> Value {
    let name = b"  %v";
    let load_instruction = b" = load ";
    let address = b", ptr %v";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &load_instruction);
    write_type(&mut *c, 2, element, false);
    append(&mut *c, 2, &address); number(&mut *c, 2, pointer); append(&mut *c, 2, &line);
    return Value { ty: element, id: id };
}
pub fn array_index(c: &mut Compiler, binding: Binding, index: Value) -> Value {
    let pointer = array_address(&mut *c, binding, index);
    return load_element(&mut *c, pointer, array_element(binding.ty));
}
fn slice_address(c: &mut Compiler, view: Value, index: Value) -> i32 {
    let name = b"  %v";
    let element = b" = getelementptr inbounds ";
    let pointer_operand = b", ptr ";
    let index_operand = b", i32 ";
    let line = b"\n";
    let base = extract_field(&mut *c, view, 0, 0);
    let pointer = fresh(&mut *c);
    let element_type = slice_element(view.ty);
    append(&mut *c, 2, &name); number(&mut *c, 2, pointer); append(&mut *c, 2, &element);
    write_scalar_type(&mut *c, 2, element_type, false);
    append(&mut *c, 2, &pointer_operand); operand(&mut *c, 2, base);
    append(&mut *c, 2, &index_operand); operand(&mut *c, 2, index);
    append(&mut *c, 2, &line);
    return pointer;
}
fn checked_slice_address(c: &mut Compiler, binding: Binding, index: Value) -> i32 {
    let view = load(&mut *c, binding.slot, binding.ty);
    let length = extract_field(&mut *c, view, 1, 1);
    check_index(&mut *c, index, length);
    return slice_address(&mut *c, view, index);
}
pub fn slice_index(c: &mut Compiler, binding: Binding, index: Value) -> Value {
    let pointer = checked_slice_address(&mut *c, binding, index);
    return load_element(&mut *c, pointer, slice_element(binding.ty));
}
fn slice_value(c: &mut Compiler, ty: i32, length: Value, pointer: i32, stack_slot: bool) -> Value {
    let name = b"  %v";
    let start = b" = insertvalue { ptr, i32 } zeroinitializer, ptr ";
    let local = b"%s";
    let temporary = b"%v";
    let end = b", 0\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &start);
    if stack_slot { append(&mut *c, 2, &local); }
    else { append(&mut *c, 2, &temporary); }
    number(&mut *c, 2, pointer); append(&mut *c, 2, &end);
    return insert_field(&mut *c, Value { ty: ty, id: id }, length, 1);
}
pub fn borrow_array(c: &mut Compiler, binding: Binding, ty: i32) -> Value {
    let count = array_count(binding.ty);
    let length = constant(&mut *c, 1, count.to_i64());
    return slice_value(&mut *c, ty, length, binding.slot, true);
}
pub fn project_slice(c: &mut Compiler, view: Value, start: Value, end: Value) -> Value {
    let extent = extract_field(&mut *c, view, 1, 1);
    // Both operands are i32. Unsigned comparisons also reject negative
    // endpoints before forming the pointer or calculating the new length.
    let start_bits = Value { ty: 7, id: start.id };
    let end_bits = Value { ty: 7, id: end.id };
    let extent_bits = Value { ty: 7, id: extent.id };
    let ordered = binary(&mut *c, 38, start_bits, end_bits);
    require_index_bounds(&mut *c, ordered);
    let inside = binary(&mut *c, 38, end_bits, extent_bits);
    require_index_bounds(&mut *c, inside);
    let pointer = slice_address(&mut *c, view, start);
    let length = binary(&mut *c, 30, end, start);
    return slice_value(&mut *c, view.ty, length, pointer, false);
}
pub fn store_element(c: &mut Compiler, pointer: i32, value: Value) -> () {
    let store_instruction = b"  store ";
    let space = b" ";
    let address = b", ptr %v";
    let line = b"\n";
    append(&mut *c, 2, &store_instruction);
    write_type(&mut *c, 2, value.ty, false);
    append(&mut *c, 2, &space); operand(&mut *c, 2, value);
    append(&mut *c, 2, &address); number(&mut *c, 2, pointer); append(&mut *c, 2, &line);
    return;
}
pub fn array_store(c: &mut Compiler, binding: Binding, index: Value, value: Value) -> () {
    let pointer = array_address(&mut *c, binding, index);
    store_element(&mut *c, pointer, value);
    return;
}
pub fn slice_store(c: &mut Compiler, binding: Binding, index: Value, value: Value) -> () {
    let pointer = checked_slice_address(&mut *c, binding, index);
    store_element(&mut *c, pointer, value);
    return;
}
pub fn borrow_record(c: &mut Compiler, binding: Binding, ty: i32) -> Value {
    if is_record_reference(binding.ty) {
        let pointer = load(&mut *c, binding.slot, binding.ty);
        return Value { ty: ty, id: pointer.id };
    }
    let name = b"  %v";
    let address = b" = getelementptr inbounds ";
    let slot_prefix = b", ptr %s";
    let end = b", i32 0\n";
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, pointer); append(&mut *c, 2, &address);
    write_type(&mut *c, 2, binding.ty, false); append(&mut *c, 2, &slot_prefix);
    number(&mut *c, 2, binding.slot); append(&mut *c, 2, &end);
    return Value { ty: ty, id: pointer };
}

pub fn byte_buffer_view(c: &mut Compiler, buffer: Value, ty: i32) -> Value {
    let prefix = b"  %v";
    let invocation = b" = call ptr @__oxid_scalar_bytebuf_view(ptr ";
    let extent = b", ptr %s";
    let end = b")\n";
    let output = slot(&mut *c, 1);
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, pointer); append(&mut *c, 2, &invocation);
    operand(&mut *c, 2, buffer); append(&mut *c, 2, &extent); number(&mut *c, 2, output);
    append(&mut *c, 2, &end);
    let count = load(&mut *c, output, 1);
    return slice_value(&mut *c, ty, count, pointer, false);
}

pub fn vector_view(c: &mut Compiler, buffer: Value, element: i32, ty: i32) -> Value {
    let prefix = b"  %v";
    let invocation = b" = call ptr @__oxid_scalar_vector_view(ptr ";
    let type_id = b", i32 ";
    let extent = b", ptr %s";
    let end = b")\n";
    let output = slot(&mut *c, 1);
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, pointer); append(&mut *c, 2, &invocation);
    operand(&mut *c, 2, buffer); append(&mut *c, 2, &type_id);
    number(&mut *c, 2, vector_runtime_tag(element)); append(&mut *c, 2, &extent);
    number(&mut *c, 2, output); append(&mut *c, 2, &end);
    let count = load(&mut *c, output, 1);
    return slice_value(&mut *c, ty, count, pointer, false);
}

pub fn vector_index(c: &mut Compiler, buffer: Value, index: Value, element: i32) -> i32 {
    let prefix = b"  %v";
    let invocation = b" = call ptr @__oxid_scalar_vector_index(ptr ";
    let integer = b", i32 ";
    let end = b")\n";
    let result = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, result); append(&mut *c, 2, &invocation);
    operand(&mut *c, 2, buffer); append(&mut *c, 2, &integer);
    number(&mut *c, 2, vector_runtime_tag(element)); append(&mut *c, 2, &integer);
    operand(&mut *c, 2, index); append(&mut *c, 2, &end);
    return result;
}

fn vector_scalar(c: &mut Compiler, value: Value) -> Value {
    if value.ty == 2 { return widen(&mut *c, value, 4); }
    return value;
}

fn packed_slot(c: &mut Compiler, bytes: i32) -> i32 {
    let prefix = b"  %s";
    let array = b" = alloca [";
    let end = b" x i8]\n";
    let id = c.next_slot;
    let cells = (bytes + 7) / 8;
    if cells > 4096 - id { c.error = 68; return 0; }
    c.next_slot = id + cells;
    append(&mut *c, 1, &prefix); number(&mut *c, 1, id);
    append(&mut *c, 1, &array); number(&mut *c, 1, bytes); append(&mut *c, 1, &end);
    return id;
}

fn pack_vector_item(c: &mut Compiler, value: Value) -> i32 {
    let invocation = b"  call void @__oxid_scalar_vector_pack(i32 ";
    let pointer = b", ptr %s";
    let end = b")\n";
    let shape = copy_record_shape(&*c, value.ty);
    let owner = slot(&mut *c, value.ty);
    store(&mut *c, owner, value);
    let packed = packed_slot(&mut *c, shape.dense);
    append(&mut *c, 2, &invocation); number(&mut *c, 2, vector_runtime_tag(value.ty));
    append(&mut *c, 2, &pointer); number(&mut *c, 2, owner);
    append(&mut *c, 2, &pointer); number(&mut *c, 2, packed); append(&mut *c, 2, &end);
    return packed;
}

pub fn vector_load(c: &mut Compiler, pointer: i32, element: i32) -> Value {
    if !is_record(element) { return load_element(&mut *c, pointer, element); }
    let invocation = b"  call void @__oxid_scalar_vector_unpack(i32 ";
    let source = b", ptr %v";
    let output = b", ptr %s";
    let end = b")\n";
    let owner = slot(&mut *c, element);
    store(&mut *c, owner, Value { ty: element, id: -1 });
    append(&mut *c, 2, &invocation); number(&mut *c, 2, vector_runtime_tag(element));
    append(&mut *c, 2, &source); number(&mut *c, 2, pointer);
    append(&mut *c, 2, &output); number(&mut *c, 2, owner); append(&mut *c, 2, &end);
    return load(&mut *c, owner, element);
}

pub fn vector_store(c: &mut Compiler, pointer: i32, value: Value) -> () {
    if is_record(value.ty) {
        let invocation = b"  call void @__oxid_scalar_vector_write(i32 ";
        let source = b", ptr %s";
        let output = b", ptr %v";
        let end = b")\n";
        let packed = pack_vector_item(&mut *c, value);
        append(&mut *c, 2, &invocation); number(&mut *c, 2, vector_runtime_tag(value.ty));
        append(&mut *c, 2, &source); number(&mut *c, 2, packed);
        append(&mut *c, 2, &output); number(&mut *c, 2, pointer); append(&mut *c, 2, &end);
        return;
    }
    let item = vector_scalar(&mut *c, value);
    store_element(&mut *c, pointer, item);
    return;
}

fn vector_operation_arguments(c: &mut Compiler, count: Value, scalar_slot: i32) -> () {
    let integer = b", i32 ";
    let zero = b"0";
    let pointer = b", ptr ";
    let local = b"%s";
    let null_pointer = b"null";
    append(&mut *c, 2, &integer);
    if count.ty == 1 { operand(&mut *c, 2, count); } else { append(&mut *c, 2, &zero); }
    append(&mut *c, 2, &pointer);
    if scalar_slot >= 0 { append(&mut *c, 2, &local); number(&mut *c, 2, scalar_slot); }
    else { append(&mut *c, 2, &null_pointer); }
    return;
}

pub fn vector_operation(c: &mut Compiler, operation: i32, buffer: Value,
    count: Value, scalar: Value, view: Value) -> Value {
    let ty = referent_record(buffer.ty);
    if operation == 8 {
        let moved = load_element(&mut *c, buffer.id, ty);
        store_element(&mut *c, buffer.id, Value { ty: ty, id: -1 });
        return moved;
    }
    let prefix = b"  %v";
    let invocation = b" = call i32 @__oxid_scalar_vector_operation(i32 ";
    let integer = b", i32 ";
    let pointer = b", ptr ";
    let null_source = b", ptr null, i32 0)\n";
    let mut scalar_slot = -1;
    if operation == 4 || operation == 6 {
        if is_record(scalar.ty) { scalar_slot = pack_vector_item(&mut *c, scalar); }
        else {
            let item = vector_scalar(&mut *c, scalar);
            scalar_slot = slot(&mut *c, item.ty);
            store(&mut *c, scalar_slot, item);
        }
    }
    let mut base = Value { ty: 0, id: -1 };
    let mut length = Value { ty: 0, id: -1 };
    if operation == 5 {
        base = extract_field(&mut *c, view, 0, 0);
        length = extract_field(&mut *c, view, 1, 1);
    }
    let tag = vector_runtime_tag(vector_element(&*c, ty));
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &invocation);
    number(&mut *c, 2, operation); append(&mut *c, 2, &integer); number(&mut *c, 2, tag);
    append(&mut *c, 2, &pointer); operand(&mut *c, 2, buffer);
    vector_operation_arguments(&mut *c, count, scalar_slot);
    if operation == 5 {
        let end = b")\n";
        append(&mut *c, 2, &pointer); operand(&mut *c, 2, base);
        append(&mut *c, 2, &integer); operand(&mut *c, 2, length); append(&mut *c, 2, &end);
    } else { append(&mut *c, 2, &null_source); }
    let score = Value { ty: 1, id: id };
    if operation == 1 || operation == 2 { return score; }
    if operation == 7 { return Value { ty: 3, id: -1 }; }
    let aggregate = Value { ty: record_type(c.growth_status), id: -1 };
    return insert_field(&mut *c, aggregate, score, 0);
}

fn drop_callee(c: &mut Compiler, destination: i32, ty: i32) -> () {
    let buffer = b"@__oxid_bytebuf_drop";
    let file = b"@__oxid_file_drop";
    let record = b"@drop_r";
    if is_byte_buffer(&*c, ty) { append(&mut *c, destination, &buffer); }
    else if is_file(&*c, ty) { append(&mut *c, destination, &file); }
    else {
        append(&mut *c, destination, &record);
        number(&mut *c, destination, record_index(ty));
    }
    return;
}

pub fn drop_owner(c: &mut Compiler, ty: i32, slot_id: i32) -> () {
    if !needs_drop(&*c, ty) { return; }
    let invocation = b"  call void ";
    let slot_prefix = b"(ptr %s";
    let end = b")\n";
    append(&mut *c, 2, &invocation); drop_callee(&mut *c, 2, ty);
    append(&mut *c, 2, &slot_prefix); number(&mut *c, 2, slot_id); append(&mut *c, 2, &end);
    return;
}

pub fn byte_buffer_operation(c: &mut Compiler, operation: i32, buffer: Value,
    size: Value, fill: Value, view: Value) -> Value {
    if operation == 10 {
        let ty = record_type(c.byte_buffer);
        let moved = load_element(&mut *c, buffer.id, ty);
        store_element(&mut *c, buffer.id, Value { ty: ty, id: -1 });
        return moved;
    }
    let prefix = b"  %v";
    let invocation = b" = call i32 @__oxid_scalar_bytebuf_operation(i32 ";
    let ptr = b", ptr ";
    let i32_parameter = b", i32 ";
    let zero = b"0";
    let empty = b"null, i32 0";
    let end = b")\n";
    let mut base = Value { ty: 0, id: -1 };
    let mut count = Value { ty: 0, id: -1 };
    if operation == 5 {
        base = extract_field(&mut *c, view, 0, 0);
        count = extract_field(&mut *c, view, 1, 1);
    }
    let mut wide_fill = Value { ty: 0, id: -1 };
    if fill.ty == 4 { wide_fill = widen(&mut *c, fill, 1); }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &invocation);
    number(&mut *c, 2, operation); append(&mut *c, 2, &ptr); operand(&mut *c, 2, buffer);
    append(&mut *c, 2, &i32_parameter);
    if size.ty == 1 { operand(&mut *c, 2, size); } else { append(&mut *c, 2, &zero); }
    append(&mut *c, 2, &i32_parameter);
    if wide_fill.ty == 1 { operand(&mut *c, 2, wide_fill); } else { append(&mut *c, 2, &zero); }
    append(&mut *c, 2, &ptr);
    if operation == 5 {
        operand(&mut *c, 2, base); append(&mut *c, 2, &i32_parameter); operand(&mut *c, 2, count);
    } else { append(&mut *c, 2, &empty); }
    append(&mut *c, 2, &end);
    let score = Value { ty: 1, id: id };
    if operation == 1 || operation == 2 { return score; }
    if operation == 7 { return Value { ty: 3, id: -1 }; }
    let aggregate = Value { ty: record_type(c.growth_status), id: -1 };
    return insert_field(&mut *c, aggregate, score, 0);
}

pub fn record_field_address(c: &mut Compiler, binding: Binding, field: i32) -> i32 {
    let borrowed = is_record_reference(binding.ty);
    let mut base = binding.slot;
    if borrowed {
        let address = load(&mut *c, binding.slot, binding.ty);
        base = address.id;
    }
    return field_address(&mut *c, record_value_type(binding.ty), base, !borrowed, field);
}

pub fn field_address(c: &mut Compiler, record: i32, base: i32, local_slot: bool, field: i32) -> i32 {
    let name = b"  %v";
    let address = b" = getelementptr inbounds ";
    let pointer_prefix = b", ptr %v";
    let slot_prefix = b", ptr %s";
    let ordinal = b", i32 0, i32 ";
    let line = b"\n";
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, pointer); append(&mut *c, 2, &address);
    write_type(&mut *c, 2, record, false);
    if local_slot { append(&mut *c, 2, &slot_prefix); }
    else { append(&mut *c, 2, &pointer_prefix); }
    number(&mut *c, 2, base); append(&mut *c, 2, &ordinal);
    number(&mut *c, 2, field); append(&mut *c, 2, &line);
    return pointer;
}

pub fn borrow_array_field(c: &mut Compiler, pointer: i32, array: i32, ty: i32) -> Value {
    let count = array_count(array);
    let length = constant(&mut *c, 1, count.to_i64());
    return slice_value(&mut *c, ty, length, pointer, false);
}

pub fn enum_switch(c: &mut Compiler, tag: Value, labels: &Vec<i32>, invalid: i32) -> () {
    let head = b"  switch i32 ";
    let target = b", label %b";
    let start = b" [\n";
    let entry = b"    i32 ";
    let line = b"\n";
    let end = b"  ]\n";
    append(&mut *c, 2, &head);
    operand(&mut *c, 2, tag);
    append(&mut *c, 2, &target); number(&mut *c, 2, invalid);
    append(&mut *c, 2, &start);
    for index in 0..vector_len(&*labels) {
        append(&mut *c, 2, &entry); number(&mut *c, 2, index);
        append(&mut *c, 2, &target); number(&mut *c, 2, labels[index]);
        append(&mut *c, 2, &line);
    }
    append(&mut *c, 2, &end);
    c.terminated = true;
    return;
}
pub fn negate_bool(c: &mut Compiler, value: Value) -> Value {
    let literal_bytes_0 = b"  %v";
    let literal_bytes_1 = b" = xor i1 ";
    let literal_bytes_2 = b", true\n";

    let id = fresh(&mut *c);
    append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, id);
    append(&mut *c, 2, &literal_bytes_1); operand(&mut *c, 2, value); append(&mut *c, 2, &literal_bytes_2);
    return Value { ty: 2, id: id };
}
fn wide_binary(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    let literal_bytes_0 = b"  %v";
    let literal_bytes_1 = b" = ";
    let call_prefix = b"call ";
    let space = b" ";
    let open = b"(";
    let comma = b", ";
    let close = b")\n";
    let literal_bytes_11 = b"icmp ";
    let literal_bytes_12 = b"eq ";
    let literal_bytes_13 = b"ne ";
    let literal_bytes_14 = b"slt ";
    let literal_bytes_15 = b"sle ";
    let literal_bytes_16 = b"sgt ";
    let literal_bytes_17 = b"sge ";
    let literal_bytes_18 = b" ";
    let literal_bytes_19 = b", ";
    let literal_bytes_20 = b"\n";

    let id = fresh(&mut *c);
    append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, id); append(&mut *c, 2, &literal_bytes_1);
    if op >= 29 && op <= 33 {
        append(&mut *c, 2, &call_prefix);
        write_type(&mut *c, 2, left.ty, false); append(&mut *c, 2, &space);
        signed_symbol(&mut *c, 2, op, left.ty); append(&mut *c, 2, &open);
        write_type(&mut *c, 2, left.ty, false); append(&mut *c, 2, &space); operand(&mut *c, 2, left);
        append(&mut *c, 2, &comma);
        write_type(&mut *c, 2, left.ty, false); append(&mut *c, 2, &space); operand(&mut *c, 2, right);
        append(&mut *c, 2, &close);
        return Value { ty: left.ty, id: id };
    }
    append(&mut *c, 2, &literal_bytes_11);
    if op == 35 { append(&mut *c, 2, &literal_bytes_12); }
    if op == 36 { append(&mut *c, 2, &literal_bytes_13); }
    if op == 37 { append(&mut *c, 2, &literal_bytes_14); }
    if op == 38 { append(&mut *c, 2, &literal_bytes_15); }
    if op == 39 { append(&mut *c, 2, &literal_bytes_16); }
    if op == 40 { append(&mut *c, 2, &literal_bytes_17); }
    write_type(&mut *c, 2, left.ty, false); append(&mut *c, 2, &literal_bytes_18); operand(&mut *c, 2, left);
    append(&mut *c, 2, &literal_bytes_19); operand(&mut *c, 2, right); append(&mut *c, 2, &literal_bytes_20);
    return Value { ty: 2, id: id };
}
pub fn widen(c: &mut Compiler, value: Value, ty: i32) -> Value {
    let prefix = b"  %v";
    let unsigned = b" = zext ";
    let signed = b" = sext ";
    let space = b" ";
    let to = b" to ";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    if value.ty == 2 || value.ty == 4 || value.ty == 7 { append(&mut *c, 2, &unsigned); }
    else { append(&mut *c, 2, &signed); }
    write_type(&mut *c, 2, value.ty, false); append(&mut *c, 2, &space);
    operand(&mut *c, 2, value); append(&mut *c, 2, &to);
    write_type(&mut *c, 2, ty, false); append(&mut *c, 2, &line);
    return Value { ty: ty, id: id };
}
pub fn narrow_long(c: &mut Compiler, value: Value, ty: i32) -> Value {
    let prefix = b"  %v";
    let call_prefix = b" = call ";
    let helper = b" @ox_from_long_";
    let arg = b"(i64 ";
    let end = b")\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    append(&mut *c, 2, &call_prefix); write_type(&mut *c, 2, ty, false);
    append(&mut *c, 2, &helper); write_type(&mut *c, 2, ty, false);
    append(&mut *c, 2, &arg); operand(&mut *c, 2, value); append(&mut *c, 2, &end);
    return Value { ty: ty, id: id };
}

pub fn narrow_byte(c: &mut Compiler, value: Value, arithmetic: bool) -> Value {
    let prefix = b"  %v";
    let call_prefix = b" = call i8 @ox_byte(i32 ";
    let overflow = b", ptr @ov, i64 17)\n";
    let conversion = b", ptr @byte_conversion, i64 29)\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix);
    number(&mut *c, 2, id);
    append(&mut *c, 2, &call_prefix);
    operand(&mut *c, 2, value);
    if arithmetic { append(&mut *c, 2, &overflow); }
    else { append(&mut *c, 2, &conversion); }
    return Value { ty: 4, id: id };
}

fn byte_binary(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    // Widening preserves unsigned comparisons and fits every byte product in i32.
    let a = widen(&mut *c, left, 1);
    let b = widen(&mut *c, right, 1);
    let value = wide_binary(&mut *c, op, a, b);
    if op <= 33 { return narrow_byte(&mut *c, value, true); }
    return value;
}

pub fn narrow_unsigned(c: &mut Compiler, value: Value, arithmetic: bool) -> Value {
    let prefix = b"  %v";
    let call_prefix = b" = call i32 @ox_unsigned(i64 ";
    let overflow = b", ptr @ov, i64 17)\n";
    let conversion = b", ptr @unsigned_conversion, i64 28)\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    append(&mut *c, 2, &call_prefix); operand(&mut *c, 2, value);
    if arithmetic { append(&mut *c, 2, &overflow); }
    else { append(&mut *c, 2, &conversion); }
    return Value { ty: 7, id: id };
}

pub fn checked_character(c: &mut Compiler, value: Value) -> Value {
    let prefix = b"  %v";
    let call_prefix = b" = call i32 @ox_character(i64 ";
    let end = b")\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    append(&mut *c, 2, &call_prefix); operand(&mut *c, 2, value);
    append(&mut *c, 2, &end);
    return Value { ty: 6, id: id };
}

fn bitwise(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    let prefix = b"  %v";
    let assign = b" = ";
    let and_instruction = b"and ";
    let or_instruction = b"or ";
    let xor_instruction = b"xor ";
    let left_shift = b"shl ";
    let signed_right_shift = b"ashr ";
    let unsigned_right_shift = b"lshr ";
    let space = b" ";
    let comma = b", ";
    let line = b"\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &assign);
    if op == 65 { append(&mut *c, 2, &and_instruction); }
    if op == 66 { append(&mut *c, 2, &or_instruction); }
    if op == 67 { append(&mut *c, 2, &xor_instruction); }
    if op == 69 { append(&mut *c, 2, &left_shift); }
    if op == 70 {
        if left.ty == 1 || left.ty == 5 { append(&mut *c, 2, &signed_right_shift); }
        else { append(&mut *c, 2, &unsigned_right_shift); }
    }
    write_type(&mut *c, 2, left.ty, false);
    append(&mut *c, 2, &space); operand(&mut *c, 2, left);
    append(&mut *c, 2, &comma); operand(&mut *c, 2, right);
    append(&mut *c, 2, &line);
    return Value { ty: left.ty, id: id };
}

pub fn shift(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    let guard = b"  call void @ox_check_shift(i64 ";
    let width_arg = b", i64 ";
    let end_call = b")\n";
    let prefix = b"  %v";
    let truncation = b" = trunc i64 ";
    let to = b" to ";
    let line = b"\n";
    let mut width = 32;
    if left.ty == 4 { width = 8; }
    if left.ty == 5 { width = 64; }
    let mut count = right;
    if right.ty != 5 { count = widen(&mut *c, right, 5); }
    append(&mut *c, 2, &guard); operand(&mut *c, 2, count);
    append(&mut *c, 2, &width_arg); number(&mut *c, 2, width); append(&mut *c, 2, &end_call);
    if left.ty != 5 {
        // The guard checks the full count before truncation for LLVM's operand type.
        let narrowed = fresh(&mut *c);
        append(&mut *c, 2, &prefix); number(&mut *c, 2, narrowed);
        append(&mut *c, 2, &truncation); operand(&mut *c, 2, count);
        append(&mut *c, 2, &to); write_type(&mut *c, 2, left.ty, false); append(&mut *c, 2, &line);
        count = Value { ty: left.ty, id: narrowed };
    }
    return bitwise(&mut *c, op, left, count);
}

pub fn binary(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    if op >= 65 && op <= 67 { return bitwise(&mut *c, op, left, right); }
    if left.ty == 4 { return byte_binary(&mut *c, op, left, right); }
    if left.ty == 7 {
        let a = widen(&mut *c, left, 5);
        let b = widen(&mut *c, right, 5);
        let value = wide_binary(&mut *c, op, a, b);
        if op <= 33 { return narrow_unsigned(&mut *c, value, true); }
        return value;
    }
    return wide_binary(&mut *c, op, left, right);
}

pub fn call(c: &mut Compiler, function: i32, args: &Vec<Value>, result: i32) -> Value {
    let literal_bytes_0 = b"  ";
    let literal_bytes_1 = b"%v";
    let literal_bytes_2 = b" = ";
    let literal_bytes_3 = b"call ";
    let literal_bytes_4 = b" @f";
    let literal_bytes_5 = b"(i32 %next_depth";
    let literal_bytes_6 = b", ";
    let literal_bytes_7 = b" ";
    let literal_bytes_8 = b")\n";

    let mut id = -1;
    append(&mut *c, 2, &literal_bytes_0);
    if result != 3 {
        id = fresh(&mut *c); append(&mut *c, 2, &literal_bytes_1); number(&mut *c, 2, id); append(&mut *c, 2, &literal_bytes_2);
    }
    append(&mut *c, 2, &literal_bytes_3); write_type(&mut *c, 2, result, true);
    append(&mut *c, 2, &literal_bytes_4); number(&mut *c, 2, function); append(&mut *c, 2, &literal_bytes_5);
    let mut i = 0;
    while i < args.len() {
        append(&mut *c, 2, &literal_bytes_6);
        let value = args[i];
        write_type(&mut *c, 2, value.ty, false); append(&mut *c, 2, &literal_bytes_7); operand(&mut *c, 2, value);
        i = i + 1;
    }
    append(&mut *c, 2, &literal_bytes_8);
    return Value { ty: result, id: id };
}
pub fn returned(c: &mut Compiler, value: Value) -> () {
    let literal_bytes_0 = b"  ret ";
    let literal_bytes_1 = b" ";
    let literal_bytes_2 = b"\n";

    append(&mut *c, 2, &literal_bytes_0); write_type(&mut *c, 2, value.ty, true);
    if value.ty != 3 { append(&mut *c, 2, &literal_bytes_1); operand(&mut *c, 2, value); }
    append(&mut *c, 2, &literal_bytes_2); c.terminated = true; c.live = false;
    return;
}
fn emit_depth_guard(c: &mut Compiler) -> () {
    // The 128-function declaration limit also bounds every acyclic call path.
    let test = b"  %depth_ok = icmp ult i32 %depth, 128\n  br i1 %depth_ok, label %start, label %depth_error\n";
    let exceeded = b"depth_error:\n  call void @__oxid_overflow(ptr @depth_error_text, i64 26)\n  unreachable\n";
    let admitted = b"start:\n  %next_depth = add nuw nsw i32 %depth, 1\n";
    append(&mut *c, 0, &test);
    append(&mut *c, 0, &exceeded);
    append(&mut *c, 0, &admitted);
    return;
}

pub fn finish_function(c: &mut Compiler, function: i32) -> () {
    let literal_bytes_0 = b"define internal ";
    let literal_bytes_1 = b" @f";
    let literal_bytes_2 = b"(i32 %depth";
    let literal_bytes_3 = b", ";
    let literal_bytes_4 = b" %a";
    let literal_bytes_5 = b") {\nentry:\n";
    let literal_bytes_6 = b"}\n";

    let f = c.functions[function];
    append(&mut *c, 0, &literal_bytes_0); write_type(&mut *c, 0, f.result, true);
    append(&mut *c, 0, &literal_bytes_1); number(&mut *c, 0, function); append(&mut *c, 0, &literal_bytes_2);
    let mut i = 0;
    while i < f.count {
        append(&mut *c, 0, &literal_bytes_3);
        let p = c.params[f.params + i];
        write_type(&mut *c, 0, p.ty, false); append(&mut *c, 0, &literal_bytes_4); number(&mut *c, 0, i);
        i = i + 1;
    }
    append(&mut *c, 0, &literal_bytes_5);
    let slots = take(&mut *c.slots);
    let body = take(&mut *c.body);
    // Entry-block slots have one fixed stack layout, including at -O0.
    append(&mut *c, 0, &slots);
    emit_depth_guard(&mut *c);
    append(&mut *c, 0, &body); append(&mut *c, 0, &literal_bytes_6);
    return;
}
fn preamble_text_0_0(c: &mut Compiler) -> () {
    if c.target_arm64 {
        let text = b"; Oxid scalar compiler 1\ntarget triple = \"aarch64-unknown-linux-gnu\"\n";
        append(&mut *c, 0, &text);
        return;
    }
    let text = b"; Oxid scalar compiler 1\ntarget triple = \"x86_64-unknown-linux-gnu\"\n";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_1_0(c: &mut Compiler) -> () {
    let text = b"declare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_1_1(c: &mut Compiler) -> () {
    let text = b"_unit()\ndeclare void @__oxid_overflow(ptr, i64) noreturn\n";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_2_0(c: &mut Compiler) -> () {
    let text = b"@ov = private constant [17 x i8] c\"integer overflow\\0A\"\n@dz = private constant [17 x i8] c\"divis";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_2_1(c: &mut Compiler) -> () {
    let text = b"ion by zero\\0A\"\n";
    append(&mut *c, 0, &text);
    return;
}
fn signed_op(c: &mut Compiler, destination: i32, op: i32) -> () {
    let add = b"add";
    let sub = b"sub";
    let mul = b"mul";
    let div = b"div";
    let rem = b"rem";
    if op == 29 { append(&mut *c, destination, &add); }
    if op == 30 { append(&mut *c, destination, &sub); }
    if op == 31 { append(&mut *c, destination, &mul); }
    if op == 32 { append(&mut *c, destination, &div); }
    if op == 33 { append(&mut *c, destination, &rem); }
    return;
}
fn signed_symbol(c: &mut Compiler, destination: i32, op: i32, ty: i32) -> () {
    let prefix = b"@ox_";
    let underscore = b"_";
    append(&mut *c, destination, &prefix); signed_op(&mut *c, destination, op);
    append(&mut *c, destination, &underscore); write_type(&mut *c, destination, ty, false);
    return;
}
fn overflow_tuple(c: &mut Compiler, ty: i32) -> () {
    let open = b"{";
    let close = b", i1} ";
    append(&mut *c, 0, &open); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &close);
    return;
}
fn intrinsic_name(c: &mut Compiler, op: i32, ty: i32) -> () {
    let prefix = b"@llvm.s";
    let suffix = b".with.overflow.";
    append(&mut *c, 0, &prefix); signed_op(&mut *c, 0, op);
    append(&mut *c, 0, &suffix); write_type(&mut *c, 0, ty, false);
    return;
}
fn signed_header(c: &mut Compiler, op: i32, ty: i32) -> () {
    let define = b"define internal ";
    let space = b" ";
    let open = b"(";
    let a = b" %a, ";
    let b = b" %b) {\nentry:\n";
    append(&mut *c, 0, &define); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &space);
    signed_symbol(&mut *c, 0, op, ty); append(&mut *c, 0, &open);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &a);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &b);
    return;
}
fn declare_intrinsic(c: &mut Compiler, op: i32, ty: i32) -> () {
    let declare = b"declare ";
    let open = b"(";
    let comma = b", ";
    let end = b")\n";
    append(&mut *c, 0, &declare); overflow_tuple(&mut *c, ty); intrinsic_name(&mut *c, op, ty);
    append(&mut *c, 0, &open); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &comma);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &end);
    return;
}
fn call_intrinsic(c: &mut Compiler, op: i32, ty: i32) -> () {
    let pair = b"  %p = call ";
    let open = b"(";
    let first = b" %a, ";
    let second = b" %b)\n  %bad = extractvalue ";
    append(&mut *c, 0, &pair); overflow_tuple(&mut *c, ty); intrinsic_name(&mut *c, op, ty);
    append(&mut *c, 0, &open); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &first);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &second); overflow_tuple(&mut *c, ty);
    return;
}
fn signed_arithmetic(c: &mut Compiler, op: i32, ty: i32) -> () {
    let test = b"%p, 1\n  br i1 %bad, label %error, label %ok\nerror:\n";
    let error = b"  call void @__oxid_overflow(ptr @ov, i64 17)\n  unreachable\nok:\n  %v = extractvalue ";
    let return_value = b"%p, 0\n  ret ";
    let end = b" %v\n}\n";
    declare_intrinsic(&mut *c, op, ty);
    signed_header(&mut *c, op, ty);
    call_intrinsic(&mut *c, op, ty);
    append(&mut *c, 0, &test); append(&mut *c, 0, &error); overflow_tuple(&mut *c, ty);
    append(&mut *c, 0, &return_value); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &end);
    return;
}
fn reject_division_zero(c: &mut Compiler, ty: i32) -> () {
    let zero = b"  %zero = icmp eq ";
    let on_zero = b" %b, 0\n  br i1 %zero, label %zero_error, label %check\nzero_error:\n";
    let error = b"  call void @__oxid_overflow(ptr @dz, i64 17)\n  unreachable\ncheck:\n";
    append(&mut *c, 0, &zero); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &on_zero);
    append(&mut *c, 0, &error);
    return;
}
fn reject_division_overflow(c: &mut Compiler, ty: i32) -> () {
    let minimum = b"  %min = icmp eq ";
    let min_int = b" %a, -2147483648\n";
    let min_long = b" %a, -9223372036854775808\n";
    let negative = b"  %negative = icmp eq ";
    let test = b" %b, -1\n  %bad = and i1 %min, %negative\n  br i1 %bad, label %error, label %ok\nerror:\n";
    append(&mut *c, 0, &minimum); write_type(&mut *c, 0, ty, false);
    if ty == 5 { append(&mut *c, 0, &min_long); } else { append(&mut *c, 0, &min_int); }
    append(&mut *c, 0, &negative); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &test);
    return;
}
fn signed_division(c: &mut Compiler, op: i32, ty: i32) -> () {
    let overflow = b"  call void @__oxid_overflow(ptr @ov, i64 17)\n  unreachable\nok:\n  %v = s";
    let space = b" ";
    let operands = b" %a, %b\n  ret ";
    let end = b" %v\n}\n";
    signed_header(&mut *c, op, ty);
    reject_division_zero(&mut *c, ty);
    reject_division_overflow(&mut *c, ty);
    append(&mut *c, 0, &overflow); signed_op(&mut *c, 0, op); append(&mut *c, 0, &space);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &operands);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &end);
    return;
}
fn long_conversion_check(c: &mut Compiler, ty: i32) -> () {
    let define = b"define internal ";
    let helper = b" @ox_from_long_";
    let truncate = b"(i64 %value) {\nentry:\n  %part = trunc i64 %value to ";
    let sign_extend = b"\n  %round = sext ";
    let zero_extend = b"\n  %round = zext ";
    let round = b" %part to i64\n  %fits = icmp eq i64 %round, %value\n";
    append(&mut *c, 0, &define); write_type(&mut *c, 0, ty, false);
    append(&mut *c, 0, &helper); write_type(&mut *c, 0, ty, false);
    append(&mut *c, 0, &truncate); write_type(&mut *c, 0, ty, false);
    if ty == 4 { append(&mut *c, 0, &zero_extend); } else { append(&mut *c, 0, &sign_extend); }
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &round);
    return;
}
fn long_conversion(c: &mut Compiler, ty: i32) -> () {
    let test = b"  br i1 %fits, label %ok, label %bad\nbad:\n  call void @__oxid_overflow(";
    let int_error = b"ptr @int_conversion, i64 28)\n";
    let byte_error = b"ptr @byte_conversion, i64 29)\n";
    let tail = b"  unreachable\nok:\n  ret ";
    let return_value = b" %part\n}\n";
    long_conversion_check(&mut *c, ty);
    append(&mut *c, 0, &test);
    if ty == 4 { append(&mut *c, 0, &byte_error); } else { append(&mut *c, 0, &int_error); }
    append(&mut *c, 0, &tail); write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &return_value);
    return;
}
fn byte_conversion_check(c: &mut Compiler) -> () {
    let message = b"@byte_conversion = private constant [29 x i8] c\"byte conversion out of range\\0A\"\n";
    let signature = b"define internal i8 @ox_byte(i32 %value, ptr %error, i64 %length) {\nentry:\n";
    let check = b"  %fits = icmp ule i32 %value, 255\n  br i1 %fits, label %ok, label %bad\n";
    append(&mut *c, 0, &message);
    append(&mut *c, 0, &signature);
    append(&mut *c, 0, &check);
    return;
}

fn byte_conversion_paths(c: &mut Compiler) -> () {
    let bad = b"bad:\n  call void @__oxid_overflow(ptr %error, i64 %length)\n  unreachable\n";
    let good = b"ok:\n  %byte = trunc i32 %value to i8\n  ret i8 %byte\n}\n";
    append(&mut *c, 0, &bad);
    append(&mut *c, 0, &good);
    return;
}

fn array_preamble(c: &mut Compiler) -> () {
    let bound = b"@index_error = private constant [26 x i8] c\"array index out of bounds\\0A\"\n";
    append(&mut *c, 0, &bound);
    byte_literals(&mut *c);
    return;
}
fn record_preamble(c: &mut Compiler) -> () {
    let prefix = b"%r";
    let declaration = b" = type { ";
    let comma = b", ";
    let end = b" }\n";
    let mut index = 0;
    while index < vector_len(&*c.records) && c.error == 0 {
        let row = c.records[index];
        if row.cells > 4096 { c.error = 68; return; }
        append(&mut *c, 0, &prefix); number(&mut *c, 0, index);
        append(&mut *c, 0, &declaration);
        if index == c.byte_buffer || index == c.file_owner || row.vector_element != 0 {
            let buffer = b"i64, i32, i32";
            let file = b"i64, i64";
            if index == c.file_owner { append(&mut *c, 0, &file); }
            else { append(&mut *c, 0, &buffer); }
            append(&mut *c, 0, &end);
            index = index + 1;
            continue;
        }
        if row.tagged {
            let tag = b"i32, ";
            append(&mut *c, 0, &tag);
        }
        if row.field_count == 0 {
            let empty = b"i8";
            append(&mut *c, 0, &empty);
        }
        let mut field = 0;
        while field < row.field_count {
            if field > 0 { append(&mut *c, 0, &comma); }
            let element = c.fields[row.first_field + field];
            write_type(&mut *c, 0, element.ty, false);
            field = field + 1;
        }
        append(&mut *c, 0, &end);
        index = index + 1;
    }
    return;
}
fn drop_record_field(c: &mut Compiler, ty: i32, tag: i32, field: i32, field_type: i32) -> () {
    let prefix = b"  %field";
    let address = b" = getelementptr inbounds ";
    let position = b", ptr %owner, i32 0, i32 ";
    let invocation = b"\n  call void ";
    let parameter = b"(ptr %field";
    let end = b")\n";
    append(&mut *c, 0, &prefix); number(&mut *c, 0, tag); append(&mut *c, 0, &address);
    write_type(&mut *c, 0, ty, false); append(&mut *c, 0, &position);
    number(&mut *c, 0, field); append(&mut *c, 0, &invocation);
    drop_callee(&mut *c, 0, field_type); append(&mut *c, 0, &parameter);
    number(&mut *c, 0, tag); append(&mut *c, 0, &end);
    return;
}

fn drop_variant_switch(c: &mut Compiler, row: Record) -> () {
    let head = b"  %tag = load i32, ptr %owner\n  switch i32 %tag, label %done [\n";
    let entry = b"    i32 ";
    let target = b", label %variant";
    let line = b"\n";
    let end = b"  ]\n";
    append(&mut *c, 0, &head);
    for index in 0..row.field_count {
        let field = c.fields[row.first_field + index];
        if needs_drop(&*c, field.ty) {
            append(&mut *c, 0, &entry); number(&mut *c, 0, index); append(&mut *c, 0, &target);
            number(&mut *c, 0, index); append(&mut *c, 0, &line);
        }
    }
    append(&mut *c, 0, &end);
    return;
}

fn drop_record_body(c: &mut Compiler, ty: i32, row: Record) -> () {
    let label_name = b"variant";
    let start = b":\n";
    let done = b"  br label %done\n";
    let exit_label = b"done:\n";
    if row.tagged {
        drop_variant_switch(&mut *c, row);
        for index in 0..row.field_count {
            let field = c.fields[row.first_field + index];
            if needs_drop(&*c, field.ty) {
                append(&mut *c, 0, &label_name); number(&mut *c, 0, index); append(&mut *c, 0, &start);
                drop_record_field(&mut *c, ty, index, index + 1, field.ty);
                append(&mut *c, 0, &done);
            }
        }
        append(&mut *c, 0, &exit_label);
    } else {
        let mut remaining = row.field_count;
        while remaining > 0 {
            remaining = remaining - 1;
            let member = c.fields[row.first_field + remaining];
            if needs_drop(&*c, member.ty) { drop_record_field(&mut *c, ty, remaining, remaining, member.ty); }
        }
    }
    return;
}

fn drop_vector_body(c: &mut Compiler, element: i32) -> () {
    let invocation = b"  call void @__oxid_vector_drop(ptr %owner, i32 ";
    let end = b")\n";
    append(&mut *c, 0, &invocation); number(&mut *c, 0, vector_runtime_tag(element));
    append(&mut *c, 0, &end);
    return;
}

fn drop_record_functions(c: &mut Compiler) -> () {
    let define = b"define internal void ";
    let signature = b"(ptr %owner) {\nentry:\n";
    let end = b"  ret void\n}\n";
    for index in 0..vector_len(&*c.records) {
        let row = c.records[index];
        if row.needs_drop && index != c.byte_buffer && index != c.file_owner {
            let ty = record_type(index);
            append(&mut *c, 0, &define); drop_callee(&mut *c, 0, ty);
            append(&mut *c, 0, &signature);
            if row.vector_element != 0 { drop_vector_body(&mut *c, row.vector_element); }
            else { drop_record_body(&mut *c, ty, row); }
            append(&mut *c, 0, &end);
        }
    }
    return;
}
fn process_preamble(c: &mut Compiler) -> () {
    let mode = b"; entry mode = process\ndeclare i32 @__oxid_process_setup()\n";
    let message = b"@status_error = private constant [44 x i8] c\"process main must return a status in 0..255\\0A\"\n";
    append(&mut *c, 0, &mode);
    append(&mut *c, 0, &message);
    return;
}

fn argument_preamble(c: &mut Compiler) -> () {
    let count = b"@argument_count = internal global i32 0, align 4\n";
    append(&mut *c, 0, &count);
    return;
}

fn argument_bytes_preamble(c: &mut Compiler) -> () {
    let start = b"declare void @__oxid_scalar_arguments_start(i32, ptr)\n";
    let bytes = b"declare i64 @__oxid_scalar_arg_bytes(i32, ptr, i32)\n";
    append(&mut *c, 0, &start); append(&mut *c, 0, &bytes);
    return;
}

pub fn argument_count(c: &mut Compiler) -> Value {
    let prefix = b"  %v";
    let load_count = b" = load i32, ptr @argument_count, align 4\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    append(&mut *c, 2, &load_count);
    return Value { ty: 1, id: id };
}

fn binary_io_preamble(c: &mut Compiler) -> () {
    let io = b"declare i64 @__oxid_scalar_binary_io(i32, ptr, i32)\n";
    append(&mut *c, 0, &io);
    return;
}

fn heap_lifetime_declarations(c: &mut Compiler) -> () {
    let lifetime = b"declare void @__oxid_bytebuf_start()\ndeclare void @__oxid_scalar_heap_finish()\n";
    let drop = b"declare void @__oxid_bytebuf_drop(ptr)\n";
    append(&mut *c, 0, &lifetime); append(&mut *c, 0, &drop);
    return;
}

fn byte_buffer_preamble(c: &mut Compiler) -> () {
    let operation = b"declare i32 @__oxid_scalar_bytebuf_operation(i32, ptr, i32, i32, ptr, i32)\n";
    let view = b"declare ptr @__oxid_scalar_bytebuf_view(ptr, ptr)\n";
    append(&mut *c, 0, &operation); append(&mut *c, 0, &view);
    return;
}
fn file_preamble(c: &mut Compiler) -> () {
    let drop = b"declare void @__oxid_file_drop(ptr)\n";
    let open_api = b"declare i32 @__oxid_scalar_file_open(i32, ptr, i32, ptr, ptr)\n";
    let io_api = b"declare i64 @__oxid_scalar_file_io(i32, ptr, ptr, i32)\n";
    append(&mut *c, 0, &drop); append(&mut *c, 0, &open_api); append(&mut *c, 0, &io_api);
    return;
}
fn file_seek_preamble(c: &mut Compiler) -> () {
    let seek_api = b"declare i64 @__oxid_scalar_file_seek(ptr, i64)\n";
    append(&mut *c, 0, &seek_api);
    return;
}
fn vector_preamble(c: &mut Compiler) -> () {
    let drop_api = b"declare void @__oxid_vector_drop(ptr, i32)\n";
    let view_api = b"declare ptr @__oxid_scalar_vector_view(ptr, i32, ptr)\n";
    let index_api = b"declare ptr @__oxid_scalar_vector_index(ptr, i32, i32)\n";
    append(&mut *c, 0, &drop_api); append(&mut *c, 0, &view_api); append(&mut *c, 0, &index_api);
    return;
}
fn vector_operation_preamble(c: &mut Compiler) -> () {
    let operation = b"declare i32 @__oxid_scalar_vector_operation(i32, i32, ptr, i32, ptr, ptr, i32)\n";
    append(&mut *c, 0, &operation);
    return;
}
fn record_vector_preamble(c: &mut Compiler) -> () {
    let pack = b"declare void @__oxid_scalar_vector_pack(i32, ptr, ptr)\n";
    let unpack = b"declare void @__oxid_scalar_vector_unpack(i32, ptr, ptr)\n";
    let write = b"declare void @__oxid_scalar_vector_write(i32, ptr, ptr)\n";
    append(&mut *c, 0, &pack); append(&mut *c, 0, &unpack); append(&mut *c, 0, &write);
    return;
}

fn transfer_call(c: &mut Compiler, input: Value, view: Value, argument: bool) -> i32 {
    let prefix = b"  %v";
    let io = b" = call i64 @__oxid_scalar_binary_io(i32 ";
    let bytes = b" = call i64 @__oxid_scalar_arg_bytes(i32 ";
    let pointer = b", ptr ";
    let size = b", i32 ";
    let line = b")\n";
    let address = extract_field(&mut *c, view, 0, 0);
    let count = extract_field(&mut *c, view, 1, 1);
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id);
    if argument { append(&mut *c, 2, &bytes); } else { append(&mut *c, 2, &io); }
    operand(&mut *c, 2, input); append(&mut *c, 2, &pointer); operand(&mut *c, 2, address);
    append(&mut *c, 2, &size); operand(&mut *c, 2, count); append(&mut *c, 2, &line);
    return id;
}

fn transfer_word(c: &mut Compiler, pair: i32, high: bool) -> Value {
    let prefix = b"  %v";
    let truncate_word = b" = trunc i64 %v";
    let end = b" to i32\n";
    let mut word_id = pair;
    if high {
        let shift_prefix = b" = lshr i64 %v";
        let width = b", 32\n";
        word_id = fresh(&mut *c);
        append(&mut *c, 2, &prefix); number(&mut *c, 2, word_id); append(&mut *c, 2, &shift_prefix);
        number(&mut *c, 2, pair); append(&mut *c, 2, &width);
    }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &truncate_word);
    number(&mut *c, 2, word_id); append(&mut *c, 2, &end);
    return Value { ty: 1, id: id };
}

pub fn binary_io(c: &mut Compiler, binding: i32, view: Value, status: i32) -> Value {
    let operation = read_some_binding() - binding;
    let selector = constant(&mut *c, 1, operation.to_i64());
    let pair = transfer_call(&mut *c, selector, view, false);
    return binary_status(&mut *c, pair, status, operation == 0);
}

fn binary_status(c: &mut Compiler, pair: i32, status: i32, reading: bool) -> Value {
    let tag = transfer_word(&mut *c, pair, false);
    let payload = transfer_word(&mut *c, pair, true);
    let initial = Value { ty: record_type(status), id: -1 };
    let tagged = insert_field(&mut *c, initial, tag, 0);
    let value = insert_field(&mut *c, tagged, payload, 1);
    let mut error_field = 2;
    if reading { error_field = 3; }
    return insert_field(&mut *c, value, payload, error_field);
}

pub fn argument_bytes(c: &mut Compiler, index: Value, view: Value) -> Value {
    let pair = transfer_call(&mut *c, index, view, true);
    let tag = transfer_word(&mut *c, pair, false);
    let payload = transfer_word(&mut *c, pair, true);
    let initial = Value { ty: record_type(c.argument_status), id: -1 };
    let tagged = insert_field(&mut *c, initial, tag, 0);
    let copied = insert_field(&mut *c, tagged, payload, 1);
    return insert_field(&mut *c, copied, payload, 2);
}

pub fn file_open(c: &mut Compiler, operation: i32, path: Value) -> Value {
    let prefix = b"  %v";
    let invocation = b" = call i32 @__oxid_scalar_file_open(i32 ";
    let pointer = b", ptr ";
    let width = b", i32 ";
    let output = b", ptr %s";
    let end = b")\n";
    let bytes = extract_field(&mut *c, path, 0, 0);
    let count = extract_field(&mut *c, path, 1, 1);
    let file_type = record_type(c.file_owner);
    let file_slot = slot(&mut *c, file_type);
    let error_slot = slot(&mut *c, 1);
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &invocation);
    number(&mut *c, 2, operation); append(&mut *c, 2, &pointer);
    operand(&mut *c, 2, bytes); append(&mut *c, 2, &width); operand(&mut *c, 2, count);
    append(&mut *c, 2, &output); number(&mut *c, 2, file_slot);
    append(&mut *c, 2, &output); number(&mut *c, 2, error_slot); append(&mut *c, 2, &end);
    let initial = Value { ty: record_type(c.file_open_result), id: -1 };
    let tagged = insert_field(&mut *c, initial, Value { ty: 1, id: id }, 0);
    let handle = load(&mut *c, file_slot, file_type);
    let opened = insert_field(&mut *c, tagged, handle, 1);
    let error = load(&mut *c, error_slot, 1);
    return insert_field(&mut *c, opened, error, 2);
}

fn file_status(c: &mut Compiler, pair: i32) -> Value {
    let tag = transfer_word(&mut *c, pair, false);
    let payload = transfer_word(&mut *c, pair, true);
    let status = Value { ty: record_type(c.file_status), id: -1 };
    let tagged = insert_field(&mut *c, status, tag, 0);
    return insert_field(&mut *c, tagged, payload, 2);
}

pub fn file_io(c: &mut Compiler, operation: i32, file: Value, bytes: Value) -> Value {
    let prefix = b"  %v";
    let invocation = b" = call i64 @__oxid_scalar_file_io(i32 ";
    let empty = b", ptr null, i32 0";
    let pointer = b", ptr ";
    let size = b", i32 ";
    let end = b")\n";
    let mut address = Value { ty: 0, id: -1 };
    let mut count = Value { ty: 0, id: -1 };
    let transfers = operation == 2 || operation == 3;
    if transfers {
        address = extract_field(&mut *c, bytes, 0, 0);
        count = extract_field(&mut *c, bytes, 1, 1);
    }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &invocation);
    number(&mut *c, 2, operation); append(&mut *c, 2, &pointer);
    operand(&mut *c, 2, file);
    if transfers {
        append(&mut *c, 2, &pointer); operand(&mut *c, 2, address);
        append(&mut *c, 2, &size); operand(&mut *c, 2, count);
    } else { append(&mut *c, 2, &empty); }
    append(&mut *c, 2, &end);
    if operation == 2 {
        let read_type = c.read_status;
        return binary_status(&mut *c, id, read_type, true);
    }
    if operation == 3 {
        let write_type_index = c.write_status;
        return binary_status(&mut *c, id, write_type_index, false);
    }
    return file_status(&mut *c, id);
}

pub fn file_seek(c: &mut Compiler, file: Value, offset: Value) -> Value {
    let prefix = b"  %v";
    let invocation = b" = call i64 @__oxid_scalar_file_seek(ptr ";
    let number_type = b", i64 ";
    let end = b")\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &prefix); number(&mut *c, 2, id); append(&mut *c, 2, &invocation);
    operand(&mut *c, 2, file); append(&mut *c, 2, &number_type); operand(&mut *c, 2, offset);
    append(&mut *c, 2, &end);
    return file_status(&mut *c, id);
}

fn preamble_scalar_printing(c: &mut Compiler) -> () {
    let print_long = b"declare i32 @__oxid_print_i64(i64)\n";
    let print_character = b"declare i32 @__oxid_print_char(i32)\n";
    append(&mut *c, 0, &print_long); append(&mut *c, 0, &print_character);
    return;
}
fn preamble_resources(c: &mut Compiler) -> () {
    let depth_error = b"@depth_error_text = private constant [26 x i8] c\"call depth limit exceeded\\0A\"\n";
    let int_error = b"@int_conversion = private constant [28 x i8] c\"i32 conversion out of range\\0A\"\n";
    preamble_text_0_0(&mut *c);
    if c.process_entry { process_preamble(&mut *c); }
    if c.uses_argument_count { argument_preamble(&mut *c); }
    if c.uses_argument_bytes { argument_bytes_preamble(&mut *c); }
    if c.uses_binary_io { binary_io_preamble(&mut *c); }
    if uses_heap(&*c) { heap_lifetime_declarations(&mut *c); }
    if c.byte_buffer >= 0 { byte_buffer_preamble(&mut *c); }
    if c.file_owner >= 0 { file_preamble(&mut *c); file_seek_preamble(&mut *c); }
    if vector_len(&*c.vector_types) > 0 {
        vector_preamble(&mut *c); vector_operation_preamble(&mut *c);
        record_vector_preamble(&mut *c);
        crate::vector_schema::write(&mut *c);
    }
    record_preamble(&mut *c);
    if uses_heap(&*c) { drop_record_functions(&mut *c); }
    preamble_text_1_0(&mut *c); preamble_text_1_1(&mut *c);
    preamble_text_2_0(&mut *c); preamble_text_2_1(&mut *c);
    append(&mut *c, 0, &depth_error); append(&mut *c, 0, &int_error);
    preamble_scalar_printing(&mut *c);
    return;
}
fn preamble_unsigned(c: &mut Compiler) -> () {
    let output = b"declare i32 @__oxid_print_u64(i64)\n";
    let uint_message = b"@unsigned_conversion = private constant [28 x i8] c\"u32 conversion out of range\\0A\"\n";
    append(&mut *c, 0, &output);
    append(&mut *c, 0, &uint_message);
    return;
}
fn conversion_return(c: &mut Compiler) -> () {
    let tail = b"  unreachable\nok:\n  %result = trunc i64 %value to i32\n  ret i32 %result\n}\n";
    append(&mut *c, 0, &tail);
    return;
}
fn preamble_unsigned_check(c: &mut Compiler) -> () {
    let check = b"define internal i32 @ox_unsigned(i64 %value, ptr %message, i64 %length) {\nentry:\n  %fits = icmp ule i64 %value, 4294967295\n  br i1 %fits, label %ok, label %bad\n";
    let fail = b"bad:\n  call void @__oxid_overflow(ptr %message, i64 %length)\n";
    append(&mut *c, 0, &check); append(&mut *c, 0, &fail);
    conversion_return(&mut *c);
    return;
}
fn preamble_character_check(c: &mut Compiler) -> () {
    let char_message = b"@character_conversion = private constant [44 x i8] c\"integer is outside the Unicode scalar range\\0A\"\n";
    let char_head = b"define internal i32 @ox_character(i64 %value) {\nentry:\n  %fits = icmp ule i64 %value, 1114111\n";
    append(&mut *c, 0, &char_message);
    append(&mut *c, 0, &char_head);
    return;
}
fn preamble_character_surrogates(c: &mut Compiler) -> () {
    let check = b"  %offset = sub i64 %value, 55296\n  %outside = icmp uge i64 %offset, 2048\n  %valid = and i1 %fits, %outside\n  br i1 %valid, label %ok, label %bad\n";
    let fail = b"bad:\n  call void @__oxid_overflow(ptr @character_conversion, i64 44)\n";
    append(&mut *c, 0, &check); append(&mut *c, 0, &fail);
    conversion_return(&mut *c);
    return;
}
fn preamble_shift_header(c: &mut Compiler) -> () {
    let message = b"@shift_error = private constant [64 x i8] c\"integer shift count must be nonnegative and below the bit width\\0A\"\n";
    let header = b"define internal void @ox_check_shift(i64 %count, i64 %width) {\nentry:\n";
    append(&mut *c, 0, &message); append(&mut *c, 0, &header);
    return;
}
fn preamble_shift_body(c: &mut Compiler) -> () {
    // An unsigned comparison also rejects the sign-extended negative counts.
    let check = b"  %fits = icmp ult i64 %count, %width\n  br i1 %fits, label %ok, label %invalid\n";
    let failure = b"invalid:\n  call void @__oxid_overflow(ptr @shift_error, i64 64)\n  unreachable\n";
    let success = b"ok:\n  ret void\n}\n";
    append(&mut *c, 0, &check); append(&mut *c, 0, &failure); append(&mut *c, 0, &success);
    return;
}
pub fn preamble(c: &mut Compiler) -> () {
    preamble_resources(&mut *c);
    preamble_unsigned(&mut *c);
    preamble_unsigned_check(&mut *c);
    preamble_character_check(&mut *c);
    preamble_character_surrogates(&mut *c);
    preamble_shift_header(&mut *c); preamble_shift_body(&mut *c);
    array_preamble(&mut *c);
    byte_conversion_check(&mut *c);
    byte_conversion_paths(&mut *c);
    long_conversion(&mut *c, 1); long_conversion(&mut *c, 4);
    let mut op = 29;
    while op <= 31 {
        signed_arithmetic(&mut *c, op, 1); signed_arithmetic(&mut *c, op, 5);
        op = op + 1;
    }
    while op <= 33 {
        signed_division(&mut *c, op, 1); signed_division(&mut *c, op, 5);
        op = op + 1;
    }
    return;
}
fn wrapper_text_0_0(c: &mut Compiler) -> () {
    if c.uses_argument_count {
        let text = b"define i32 @main(i32 %argc, ptr %argv) {\nentry:\n  ";
        append(&mut *c, 0, &text);
        return;
    }
    let text = b"define i32 @main() {\nentry:\n  ";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_1_0(c: &mut Compiler) -> () {
    let text = b"%value = ";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_2_0(c: &mut Compiler) -> () {
    let text = b"call ";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_3_0(c: &mut Compiler) -> () {
    let text = b" @f";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_4_0(c: &mut Compiler) -> () {
    let text = b"(i32 0)\n";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_5_0(c: &mut Compiler) -> () {
    let text = b"  %status = call i32 @__oxid_print_i32(i32 %value)\n";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_6_0(c: &mut Compiler) -> () {
    let text = b"  %wide = zext i1 %value to i32\n  %status = call i32 @__oxid_print_bool(i32 %wide)\n";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_7_0(c: &mut Compiler) -> () {
    let text = b"  %status = call i32 @__oxid_print_unit()\n";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_byte_result(c: &mut Compiler) -> () {
    let text = b"  %wide = zext i8 %value to i32\n  %status = call i32 @__oxid_print_i32(i32 %wide)\n";
    append(&mut *c, 0, &text);
    return;
}
fn wrapper_text_8_0(c: &mut Compiler) -> () {
    let text = b"  ret i32 %status\n}\n; Oxid scalar compiler end\n";
    append(&mut *c, 0, &text);
    return;
}
fn process_setup(c: &mut Compiler) -> () {
    let setup = b"%setup = call i32 @__oxid_process_setup()\n  %ready = icmp eq i32 %setup, 0\n";
    let edges = b"  br i1 %ready, label %process, label %setup_error\nsetup_error:\n  ret i32 74\nprocess:\n  ";
    append(&mut *c, 0, &setup); append(&mut *c, 0, &edges);
    return;
}
fn process_exit(c: &mut Compiler) -> () {
    let check = b"  %accepted = icmp ule i32 %value, 255\n  br i1 %accepted, label %exit, label %bad_status\n";
    let failure = b"bad_status:\n  call void @__oxid_overflow(ptr @status_error, i64 44)\n  unreachable\n";
    let success = b"exit:\n  ret i32 %value\n}\n; Oxid scalar compiler end\n";
    append(&mut *c, 0, &check); append(&mut *c, 0, &failure); append(&mut *c, 0, &success);
    return;
}
fn install_argument_count(c: &mut Compiler) -> () {
    let count = b"store i32 %argc, ptr @argument_count, align 4\n  ";
    append(&mut *c, 0, &count);
    if c.uses_argument_bytes {
        let start = b"call void @__oxid_scalar_arguments_start(i32 %argc, ptr %argv)\n  ";
        append(&mut *c, 0, &start);
    }
    return;
}
fn heap_lifetime(c: &mut Compiler, start: bool) -> () {
    if !uses_heap(&*c) { return; }
    if start {
        let begin = b"call void @__oxid_bytebuf_start()\n  ";
        append(&mut *c, 0, &begin);
    } else {
        let finish = b"  call void @__oxid_scalar_heap_finish()\n";
        append(&mut *c, 0, &finish);
    }
    return;
}
pub fn wrapper(c: &mut Compiler) -> () {
    let print_long = b"  %status = call i32 @__oxid_print_i64(i64 %value)\n";
    let print_character = b"  %status = call i32 @__oxid_print_char(i32 %value)\n";
    let print_unsigned = b"  %wide = zext i32 %value to i64\n  %status = call i32 @__oxid_print_u64(i64 %wide)\n";

    let entry = c.main;
    let f = c.functions[entry];
    wrapper_text_0_0(&mut *c);
    if c.process_entry { process_setup(&mut *c); }
    if c.uses_argument_count { install_argument_count(&mut *c); }
    heap_lifetime(&mut *c, true);
    if f.result != 3 { wrapper_text_1_0(&mut *c); }
    wrapper_text_2_0(&mut *c); write_type(&mut *c, 0, f.result, true);
    wrapper_text_3_0(&mut *c); number(&mut *c, 0, entry); wrapper_text_4_0(&mut *c);
    heap_lifetime(&mut *c, false);
    if c.process_entry { process_exit(&mut *c); return; }
    if f.result == 1 { wrapper_text_5_0(&mut *c); }
    if f.result == 2 { wrapper_text_6_0(&mut *c); }
    if f.result == 3 { wrapper_text_7_0(&mut *c); }
    if f.result == 4 { wrapper_byte_result(&mut *c); }
    if f.result == 5 { append(&mut *c, 0, &print_long); }
    if f.result == 6 { append(&mut *c, 0, &print_character); }
    if f.result == 7 { append(&mut *c, 0, &print_unsigned); }
    wrapper_text_8_0(&mut *c);
    return;
}
