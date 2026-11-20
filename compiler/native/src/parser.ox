use crate::model::Compiler;
use crate::model::Value;
use crate::model::Binding;
use crate::model::grown;
use crate::model::token;
use crate::model::kind;
use crate::model::expect;
use crate::model::fail;
use crate::model::local;
use crate::model::function;
use crate::model::function_binding;
use crate::model::function_visible;
use crate::model::path_module;
use crate::model::is_byte_array;
use crate::model::byte_count;
use crate::model::record;
use crate::model::record_type;
use crate::model::record_index;
use crate::model::is_record;
use crate::model::record_field;
use crate::model::word;
use crate::headers::ty as parse_type;
use crate::headers::path_name;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;
use alloc::vec::truncate;

fn invalid() -> Value { return Value { ty: 0, id: 0 }; }
fn precedence(op: i32) -> i32 {
    if op == 42 { return 1; }
    if op == 41 { return 2; }
    if op == 35 || op == 36 { return 3; }
    if op >= 37 && op <= 40 { return 4; }
    if op == 29 || op == 30 { return 5; }
    if op >= 31 && op <= 33 { return 6; }
    return 0;
}
fn bind(c: &mut Compiler, name: i32, ty: i32, writable: bool, slot: i32) -> () {
    let owner = c.functions[c.current];
    if local(&*c, name) >= 0 || function_binding(&*c, owner.source_module, name) >= 0 { fail(&mut *c); return; }
    if len(&*c.vars) >= 4096 { c.error = 68; return; }
    let row = Binding { name: name, ty: ty, slot: slot, writable: writable };
    if !grown(try_push(&mut *c.vars, row)) { c.error = 68; }
    return;
}
fn called(c: &mut Compiler, target: i32, depth: i32) -> Value {
    if target < 0 { fail(&mut *c); return invalid(); }
    let f = c.functions[target];
    let mut args = new::<Value>();
    expect(&mut *c, 20);
    let mut count = 0;
    while kind(&*c) != 21 && c.error == 0 {
        if count >= f.count { fail(&mut *c); return invalid(); }
        let value = expression(&mut *c, 1, depth + 1);
        let expected = c.params[f.params + count];
        if value.ty != expected.ty { fail(&mut *c); return invalid(); }
        if !grown(try_push(&mut args, value)) { c.error = 68; return invalid(); }
        count = count + 1;
        if kind(&*c) != 21 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 21);
    if count != f.count { fail(&mut *c); return invalid(); }
    if c.call_sites >= 4096 { c.error = 68; return invalid(); }
    c.call_sites = c.call_sites + 1;
    return crate::emit::call(&mut *c, target, &args, f.result);
}
fn qualified_call(c: &mut Compiler, depth: i32) -> Value {
    let path_start = c.cursor + 2;
    let name = path_name(&mut *c);
    if c.error != 0 { return invalid(); }
    let caller = c.functions[c.current];
    let source_module = path_module(&*c, path_start, name, caller.source_module);
    if source_module < 0 { fail(&mut *c); return invalid(); }
    let target = function(&*c, source_module, name);
    if !function_visible(&*c, caller.source_module, target) { fail(&mut *c); return invalid(); }
    return called(&mut *c, target, depth);
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
    }
    if word(&*c.source, method.start, method.end, &narrow_byte) {
        if value.ty == 4 { return value; }
        if value.ty == 1 { return crate::emit::narrow_byte(&mut *c, value, false); }
        if value.ty == 5 { return crate::emit::narrow_long(&mut *c, value, 4); }
    }
    fail(&mut *c);
    return invalid();
}
fn array_length(c: &mut Compiler, ty: i32) -> Value {
    let len_name = b"len";
    expect(&mut *c, 43);
    let method = token(&*c, c.cursor);
    expect(&mut *c, 1); expect(&mut *c, 20); expect(&mut *c, 21);
    if !word(&*c.source, method.start, method.end, &len_name) { fail(&mut *c); return invalid(); }
    let length = byte_count(ty);
    return crate::emit::constant(&mut *c, 1, length.to_i64());
}
fn record_literal(c: &mut Compiler, index: i32, depth: i32) -> Value {
    let row = c.records[index];
    let mut seen = new::<bool>();
    while len(&seen) < row.field_count {
        if !grown(try_push(&mut seen, false)) { c.error = 68; return invalid(); }
    }
    expect(&mut *c, 22);
    let mut count = 0;
    let mut aggregate = Value { ty: record_type(index), id: -1 };
    while kind(&*c) != 23 && c.error == 0 {
        let field = record_field(&*c, row, c.cursor);
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

fn field_value(c: &mut Compiler, binding: Binding) -> Value {
    expect(&mut *c, 43);
    let row = c.records[record_index(binding.ty)];
    let field = record_field(&*c, row, c.cursor);
    expect(&mut *c, 1);
    if field < 0 { fail(&mut *c); return invalid(); }
    let descriptor = c.fields[row.first_field + field];
    let aggregate = crate::emit::load(&mut *c, binding.slot, binding.ty);
    return crate::emit::extract_field(&mut *c, aggregate, field, descriptor.ty);
}

fn named_value(c: &mut Compiler, depth: i32) -> Value {
    let name = c.cursor;
    c.cursor = c.cursor + 1;
    let owner = c.functions[c.current];
    if kind(&*c) == 20 {
        let target = function_binding(&*c, owner.source_module, name);
        return called(&mut *c, target, depth);
    }
    let position = local(&*c, name);
    if kind(&*c) == 22 {
        let separator = token(&*c, c.cursor + 2);
        if position < 0 || separator.kind == 24 {
            let index = record(&*c, owner.source_module, name);
            if index >= 0 { return record_literal(&mut *c, index, depth); }
        }
    }
    if position < 0 { fail(&mut *c); return invalid(); }
    let binding = c.vars[position];
    if kind(&*c) == 51 {
        if !is_byte_array(binding.ty) { fail(&mut *c); return invalid(); }
        expect(&mut *c, 51);
        let index = expression(&mut *c, 1, depth + 1);
        expect(&mut *c, 52);
        if index.ty != 1 { fail(&mut *c); return invalid(); }
        return crate::emit::byte_index(&mut *c, binding, index);
    }
    if is_byte_array(binding.ty) && kind(&*c) == 43 { return array_length(&mut *c, binding.ty); }
    if is_record(binding.ty) && kind(&*c) == 43 { return field_value(&mut *c, binding); }
    let value = crate::emit::load(&mut *c, binding.slot, binding.ty);
    if kind(&*c) == 43 { return convert_binding(&mut *c, value); }
    return value;
}
fn byte_array_values(c: &mut Compiler, depth: i32) -> Value {
    expect(&mut *c, 51);
    let mut values = new::<Value>();
    while kind(&*c) != 52 && c.error == 0 {
        if len(&values) >= 1024 { c.error = 68; return invalid(); }
        let value = expression(&mut *c, 1, depth + 1);
        if value.ty != 4 { fail(&mut *c); return invalid(); }
        if !grown(try_push(&mut values, value)) { c.error = 68; return invalid(); }
        if kind(&*c) != 52 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 52);
    if len(&values) == 0 { fail(&mut *c); return invalid(); }
    return crate::emit::byte_array_values(&mut *c, &values);
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
    if t.kind == 50 {
        let index = c.cursor;
        c.cursor = c.cursor + 1;
        return crate::emit::byte_literal(&mut *c, index, t.decoded_length);
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
    if t.kind == 51 { return byte_array_values(&mut *c, depth); }
    if t.kind == 44 { return qualified_call(&mut *c, depth); }
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
        let op = kind(&*c);
        let rank = precedence(op);
        if rank < minimum || rank == 0 { return left; }
        c.cursor = c.cursor + 1;
        if op == 41 || op == 42 {
            left = short_circuit(&mut *c, op, left, rank, depth);
        } else {
            let right = expression(&mut *c, rank + 1, depth + 1);
            if left.ty != right.ty || left.ty == 3 || left.ty == 0
                || is_byte_array(left.ty) || is_record(left.ty) { fail(&mut *c); return invalid(); }
            if op != 35 && op != 36 && left.ty != 1 && left.ty != 4 && left.ty != 5 { fail(&mut *c); return invalid(); }
            if left.ty == 4 { left = crate::emit::byte_binary(&mut *c, op, left, right); }
            else { left = crate::emit::binary(&mut *c, op, left, right); }
        }
    }
    return invalid();
}
fn declared(c: &mut Compiler) -> () {
    expect(&mut *c, 4);
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
    let value = expression(&mut *c, 1, 0);
    if expected != 0 && value.ty != expected { fail(&mut *c); return; }
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
    let value = expression(&mut *c, 1, 0);
    if value.ty != binding.ty { fail(&mut *c); return; }
    expect(&mut *c, 25);
    crate::emit::store(&mut *c, binding.slot, value);
    return;
}
fn assign_field(c: &mut Compiler) -> () {
    let position = local(&*c, c.cursor);
    if position < 0 { fail(&mut *c); return; }
    let binding = c.vars[position];
    if !binding.writable || !is_record(binding.ty) { fail(&mut *c); return; }
    let row = c.records[record_index(binding.ty)];
    c.cursor = c.cursor + 2;
    let field = record_field(&*c, row, c.cursor);
    expect(&mut *c, 1); expect(&mut *c, 28);
    if field < 0 { fail(&mut *c); return; }
    let descriptor = c.fields[row.first_field + field];
    let value = expression(&mut *c, 1, 0);
    if value.ty != descriptor.ty { fail(&mut *c); return; }
    expect(&mut *c, 25);
    let aggregate = crate::emit::load(&mut *c, binding.slot, binding.ty);
    let updated = crate::emit::insert_field(&mut *c, aggregate, value, field);
    crate::emit::store(&mut *c, binding.slot, updated);
    return;
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
    if !binding.writable || !is_byte_array(binding.ty) { fail(&mut *c); return; }
    let begin = c.cursor + 2;
    // Like other Oxid array assignments, evaluate the right side before the index.
    c.cursor = end + 2;
    let value = expression(&mut *c, 1, 0);
    if value.ty != 4 { fail(&mut *c); return; }
    expect(&mut *c, 25);
    if c.error != 0 { return; }
    let continuation = c.cursor;
    c.cursor = begin;
    let index = expression(&mut *c, 1, 0);
    if index.ty != 1 || c.cursor != end { fail(&mut *c); return; }
    c.cursor = continuation;
    crate::emit::byte_store(&mut *c, binding, index, value);
    return;
}
fn if_statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    if depth > 64 { c.error = 68; return; }
    expect(&mut *c, 7);
    let before = c.live;
    let condition = expression(&mut *c, 1, 0);
    if condition.ty != 2 { fail(&mut *c); return; }
    let yes = crate::emit::new_label(&mut *c);
    let no = crate::emit::new_label(&mut *c);
    let merge = crate::emit::new_label(&mut *c);
    crate::emit::conditional(&mut *c, condition, yes, no);
    crate::emit::label(&mut *c, yes); c.live = before;
    block(&mut *c, break_to, continue_to, depth + 1);
    let left_falls = c.live;
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, no); c.live = before;
    if kind(&*c) == 8 {
        c.cursor = c.cursor + 1;
        if kind(&*c) == 7 { if_statement(&mut *c, break_to, continue_to, depth + 1); }
        else { block(&mut *c, break_to, continue_to, depth + 1); }
    }
    let right_falls = c.live;
    crate::emit::branch(&mut *c, merge);
    crate::emit::label(&mut *c, merge);
    c.live = left_falls || right_falls;
    return;
}
fn while_statement(c: &mut Compiler, depth: i32) -> () {
    expect(&mut *c, 9);
    let before = c.live;
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
    if ty != limit.ty || (ty != 1 && ty != 4 && ty != 5) { fail(&mut *c); return; }

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
    let mut has_next = Value { ty: 2, id: 0 };
    if ty == 4 { has_next = crate::emit::byte_binary(&mut *c, 37, index, limit); }
    else { has_next = crate::emit::binary(&mut *c, 37, index, limit); }
    crate::emit::conditional(&mut *c, has_next, body, end);

    crate::emit::label(&mut *c, body); c.live = before;
    let saved = len(&*c.vars);
    bind(&mut *c, name, ty, false, slot);
    block(&mut *c, end, advance, depth + 1);
    truncate(&mut *c.vars, saved);
    crate::emit::branch(&mut *c, advance);

    crate::emit::label(&mut *c, advance);
    let one = crate::emit::constant(&mut *c, ty, 1i64);
    let mut next = Value { ty: ty, id: 0 };
    if ty == 4 { next = crate::emit::byte_binary(&mut *c, 29, index, one); }
    else { next = crate::emit::binary(&mut *c, 29, index, one); }
    crate::emit::store(&mut *c, slot, next);
    crate::emit::branch(&mut *c, test);
    crate::emit::label(&mut *c, end); c.live = before;
    return;
}
fn statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    crate::emit::ensure(&mut *c);
    let k = kind(&*c);
    if k == 4 { declared(&mut *c); return; }
    if k == 7 { if_statement(&mut *c, break_to, continue_to, depth); return; }
    if k == 9 { while_statement(&mut *c, depth); return; }
    if k == 55 { for_statement(&mut *c, depth); return; }
    if k == 22 { block(&mut *c, break_to, continue_to, depth + 1); return; }
    if k == 10 || k == 11 {
        if break_to < 0 || continue_to < 0 { fail(&mut *c); return; }
        c.cursor = c.cursor + 1; expect(&mut *c, 25);
        if k == 10 { crate::emit::branch(&mut *c, break_to); }
        else { crate::emit::branch(&mut *c, continue_to); }
        c.live = false; return;
    }
    if k == 6 {
        c.cursor = c.cursor + 1;
        let mut value = Value { ty: 3, id: -1 };
        if kind(&*c) != 25 { value = expression(&mut *c, 1, 0); }
        expect(&mut *c, 25);
        let f = c.functions[c.current];
        if value.ty != f.result { fail(&mut *c); return; }
        crate::emit::returned(&mut *c, value); return;
    }
    let next = token(&*c, c.cursor + 1);
    if k == 1 && next.kind == 28 { assigned(&mut *c); return; }
    if k == 1 && next.kind == 43 {
        let assign = token(&*c, c.cursor + 3);
        if assign.kind == 28 { assign_field(&mut *c); return; }
    }
    if k == 1 && next.kind == 51 {
        let end = index_assignment_end(&*c, c.cursor + 1);
        if end >= 0 { index_assignment(&mut *c, end); return; }
    }
    expression(&mut *c, 1, 0); expect(&mut *c, 25);
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
        c.current = i; c.cursor = f.begin; c.next_value = 0; c.next_label = 0;
        c.next_slot = 0; c.live = true; c.terminated = false;
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
            crate::emit::returned(&mut *c, Value { ty: 3, id: -1 });
        } else {
            if !c.terminated { crate::emit::append(&mut *c, 2, &literal_bytes_4); c.terminated = true; }
        }
        crate::emit::finish_function(&mut *c, i);
        i = i + 1;
    }
    return;
}
