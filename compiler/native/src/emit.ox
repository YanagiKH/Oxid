use crate::model::Compiler;
use crate::model::Value;
use crate::model::grown;
use crate::model::Binding;
use crate::model::Token;
use crate::model::byte_array;
use crate::model::is_byte_array;
use crate::model::byte_count;
use crate::model::code;
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
pub fn write_type(c: &mut Compiler, destination: i32, ty: i32, result: bool) -> () {
    let literal_bytes_0 = b"i32";
    let literal_bytes_1 = b"void";
    let literal_bytes_2 = b"i1";
    let byte_type = b"i8";
    let long_type = b"i64";

    if ty == 1 { append(&mut *c, destination, &literal_bytes_0); return; }
    if ty == 4 { append(&mut *c, destination, &byte_type); return; }
    if ty == 5 { append(&mut *c, destination, &long_type); return; }
    if is_byte_array(ty) {
        let open = b"[";
        let end = b" x i8]";
        append(&mut *c, destination, &open);
        number(&mut *c, destination, byte_count(ty));
        append(&mut *c, destination, &end);
        return;
    }
    if ty == 3 && result { append(&mut *c, destination, &literal_bytes_1); }
    else { append(&mut *c, destination, &literal_bytes_2); }
    return;
}
pub fn operand(c: &mut Compiler, destination: i32, value: Value) -> () {
    let literal_bytes_0 = b"false";
    let literal_bytes_1 = b"%v";

    if value.ty == 3 { append(&mut *c, destination, &literal_bytes_0); }
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
    let mut cells = 1;
    if is_byte_array(ty) {
        cells = (byte_count(ty) + 7) / 8;
        if cells == 0 { cells = 1; }
    }
    if cells > 4096 - id { c.error = 68; return 0; }
    c.next_slot = id + cells;
    append(&mut *c, 1, &literal_bytes_0); number(&mut *c, 1, id);
    append(&mut *c, 1, &literal_bytes_1); write_type(&mut *c, 1, ty, false);
    append(&mut *c, 1, &literal_bytes_2);
    return id;
}
pub fn store(c: &mut Compiler, position: i32, value: Value) -> () {
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
fn literal_constant(c: &mut Compiler, index: i32, row: Token) -> () {
    let prefix = b" = private constant ";
    let data = b" c\"";
    let end = b"\"\n";
    literal_name(&mut *c, 0, index);
    append(&mut *c, 0, &prefix);
    write_type(&mut *c, 0, byte_array(row.decoded_length), false);
    append(&mut *c, 0, &data);
    let mut at = row.decoded_start;
    while at < row.decoded_start + row.decoded_length && c.error == 0 {
        let value = code(&*c.literals, at);
        let escaped = [92u8, hex_byte(value / 16), hex_byte(value % 16)];
        append(&mut *c, 0, &escaped);
        at = at + 1;
    }
    append(&mut *c, 0, &end);
    return;
}
fn byte_literals(c: &mut Compiler) -> () {
    let mut index = 0;
    while index < vector_len(&*c.tokens) && c.error == 0 {
        let row = c.tokens[index];
        if row.kind == 50 { literal_constant(&mut *c, index, row); }
        index = index + 1;
    }
    return;
}
pub fn byte_literal(c: &mut Compiler, index: i32, length: i32) -> Value {
    let name = b"  %v";
    let load_instruction = b" = load ";
    let pointer = b", ptr ";
    let end = b", align 1\n";
    let ty = byte_array(length);
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &load_instruction);
    write_type(&mut *c, 2, ty, false); append(&mut *c, 2, &pointer);
    literal_name(&mut *c, 2, index); append(&mut *c, 2, &end);
    return Value { ty: ty, id: id };
}
pub fn byte_array_values(c: &mut Compiler, values: &Vec<Value>) -> Value {
    let name = b"  %v";
    let insert = b" = insertvalue ";
    let first = b" undef";
    let space = b" ";
    let byte = b", i8 ";
    let comma = b", ";
    let line = b"\n";
    let count = vector_len(&*values);
    let ty = byte_array(count);
    let mut array = Value { ty: ty, id: -1 };
    let mut index = 0;
    while index < count && c.error == 0 {
        let id = fresh(&mut *c);
        append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &insert);
        write_type(&mut *c, 2, ty, false);
        if index == 0 { append(&mut *c, 2, &first); }
        else { append(&mut *c, 2, &space); operand(&mut *c, 2, array); }
        append(&mut *c, 2, &byte); operand(&mut *c, 2, values[index]);
        append(&mut *c, 2, &comma); number(&mut *c, 2, index); append(&mut *c, 2, &line);
        array = Value { ty: ty, id: id };
        index = index + 1;
    }
    return array;
}
fn check_byte_index(c: &mut Compiler, index: Value, length: i32) -> () {
    let name = b"  %v";
    let compare = b" = icmp ult i32 ";
    let comma = b", ";
    let line = b"\n";
    let failure = b"  call void @__oxid_overflow(ptr @index_error, i64 26)\n  unreachable\n";
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &compare);
    operand(&mut *c, 2, index); append(&mut *c, 2, &comma);
    number(&mut *c, 2, length); append(&mut *c, 2, &line);
    let valid = new_label(&mut *c);
    let invalid = new_label(&mut *c);
    conditional(&mut *c, Value { ty: 2, id: id }, valid, invalid);
    label(&mut *c, invalid);
    append(&mut *c, 2, &failure);
    label(&mut *c, valid);
    return;
}
fn byte_address(c: &mut Compiler, binding: Binding, index: Value) -> i32 {
    let name = b"  %v";
    let element = b" = getelementptr inbounds ";
    let slot_prefix = b", ptr %s";
    let ordinal = b", i32 0, i32 ";
    let line = b"\n";
    check_byte_index(&mut *c, index, byte_count(binding.ty));
    let pointer = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, pointer); append(&mut *c, 2, &element);
    write_type(&mut *c, 2, binding.ty, false); append(&mut *c, 2, &slot_prefix);
    number(&mut *c, 2, binding.slot); append(&mut *c, 2, &ordinal);
    operand(&mut *c, 2, index); append(&mut *c, 2, &line);
    return pointer;
}
pub fn byte_index(c: &mut Compiler, binding: Binding, index: Value) -> Value {
    let name = b"  %v";
    let load_instruction = b" = load i8, ptr %v";
    let alignment = b", align 1\n";
    let pointer = byte_address(&mut *c, binding, index);
    let id = fresh(&mut *c);
    append(&mut *c, 2, &name); number(&mut *c, 2, id); append(&mut *c, 2, &load_instruction);
    number(&mut *c, 2, pointer); append(&mut *c, 2, &alignment);
    return Value { ty: 4, id: id };
}
pub fn byte_store(c: &mut Compiler, binding: Binding, index: Value, value: Value) -> () {
    let store_instruction = b"  store i8 ";
    let address = b", ptr %v";
    let alignment = b", align 1\n";
    let pointer = byte_address(&mut *c, binding, index);
    append(&mut *c, 2, &store_instruction); operand(&mut *c, 2, value);
    append(&mut *c, 2, &address); number(&mut *c, 2, pointer); append(&mut *c, 2, &alignment);
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
pub fn binary(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
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
    if value.ty == 4 { append(&mut *c, 2, &unsigned); } else { append(&mut *c, 2, &signed); }
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

pub fn byte_binary(c: &mut Compiler, op: i32, left: Value, right: Value) -> Value {
    // Widening preserves unsigned comparisons and fits every byte product in i32.
    let a = widen(&mut *c, left, 1);
    let b = widen(&mut *c, right, 1);
    let value = binary(&mut *c, op, a, b);
    if op <= 33 { return narrow_byte(&mut *c, value, true); }
    return value;
}

pub fn call(c: &mut Compiler, function: i32, args: &[Value], result: i32) -> Value {
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
pub fn preamble(c: &mut Compiler) -> () {
    let depth_error = b"@depth_error_text = private constant [26 x i8] c\"call depth limit exceeded\\0A\"\n";
    let int_error = b"@int_conversion = private constant [28 x i8] c\"i32 conversion out of range\\0A\"\n";
    let print_long = b"declare i32 @__oxid_print_i64(i64)\n";
    preamble_text_0_0(&mut *c);
    preamble_text_1_0(&mut *c); preamble_text_1_1(&mut *c);
    preamble_text_2_0(&mut *c); preamble_text_2_1(&mut *c);
    append(&mut *c, 0, &depth_error); append(&mut *c, 0, &int_error); append(&mut *c, 0, &print_long);
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
pub fn wrapper(c: &mut Compiler) -> () {
    let print_long = b"  %status = call i32 @__oxid_print_i64(i64 %value)\n";

    let entry = c.main;
    let f = c.functions[entry];
    wrapper_text_0_0(&mut *c);
    if f.result != 3 { wrapper_text_1_0(&mut *c); }
    wrapper_text_2_0(&mut *c); write_type(&mut *c, 0, f.result, true);
    wrapper_text_3_0(&mut *c); number(&mut *c, 0, entry); wrapper_text_4_0(&mut *c);
    if f.result == 1 { wrapper_text_5_0(&mut *c); }
    if f.result == 2 { wrapper_text_6_0(&mut *c); }
    if f.result == 3 { wrapper_text_7_0(&mut *c); }
    if f.result == 4 { wrapper_byte_result(&mut *c); }
    if f.result == 5 { append(&mut *c, 0, &print_long); }
    wrapper_text_8_0(&mut *c);
    return;
}
