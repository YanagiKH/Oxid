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
use crate::model::edge;
use crate::model::code as byte_code;
use crate::model::set_byte;
use crate::headers::ty as parse_type;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;
use alloc::vec::truncate;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_resize;

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
    if local(&*c, name) >= 0 || function(&*c, name) >= 0 { fail(&mut *c); return; }
    if len(&*c.vars) >= 4096 { c.error = 68; return; }
    let row = Binding { name: name, ty: ty, slot: slot, writable: writable };
    if !grown(try_push(&mut *c.vars, row)) { c.error = 68; }
    return;
}
fn called(c: &mut Compiler, name: i32, depth: i32) -> Value {
    let target = function(&*c, name);
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
    edge(&mut *c, target);
    return crate::emit::call(&mut *c, target, &args, f.result);
}
fn prefix(c: &mut Compiler, depth: i32) -> Value {
    if c.error != 0 { return invalid(); }
    if depth > 64 { c.error = 68; return invalid(); }
    let t = token(&*c, c.cursor);
    if t.kind == 30 {
        c.cursor = c.cursor + 1;
        let next = token(&*c, c.cursor);
        if next.kind == 2 {
            c.cursor = c.cursor + 1;
            return crate::emit::constant(&mut *c, 1, next.value);
        }
        let negative_operand = prefix(&mut *c, depth + 1);
        if negative_operand.ty != 1 { fail(&mut *c); return invalid(); }
        let zero = crate::emit::constant(&mut *c, 1, 0);
        return crate::emit::binary(&mut *c, 30, zero, negative_operand);
    }
    if t.kind == 34 {
        c.cursor = c.cursor + 1;
        let not_operand = prefix(&mut *c, depth + 1);
        if not_operand.ty != 2 { fail(&mut *c); return invalid(); }
        return crate::emit::negate_bool(&mut *c, not_operand);
    }
    if t.kind == 2 {
        if t.value == -2147483648 { fail(&mut *c); return invalid(); }
        c.cursor = c.cursor + 1;
        return crate::emit::constant(&mut *c, 1, 0 - t.value);
    }
    if t.kind == 12 || t.kind == 13 {
        c.cursor = c.cursor + 1;
        let mut boolean_literal = 0;
        if t.kind == 12 { boolean_literal = 1; }
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
    if t.kind == 1 {
        let name = c.cursor;
        c.cursor = c.cursor + 1;
        if kind(&*c) == 20 { return called(&mut *c, name, depth); }
        let position = local(&*c, name);
        if position < 0 { fail(&mut *c); return invalid(); }
        let binding = c.vars[position];
        return crate::emit::load(&mut *c, binding.slot, binding.ty);
    }
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
    let mut bit = 0;
    if op == 42 { bit = 1; }
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
            if left.ty != right.ty || left.ty == 3 || left.ty == 0 { fail(&mut *c); return invalid(); }
            if op != 35 && op != 36 && left.ty != 1 { fail(&mut *c); return invalid(); }
            left = crate::emit::binary(&mut *c, op, left, right);
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
    if kind(&*c) == 24 { c.cursor = c.cursor + 1; expected = parse_type(&mut *c); }
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
fn statement(c: &mut Compiler, break_to: i32, continue_to: i32, depth: i32) -> () {
    crate::emit::ensure(&mut *c);
    let k = kind(&*c);
    if k == 4 { declared(&mut *c); return; }
    if k == 7 { if_statement(&mut *c, break_to, continue_to, depth); return; }
    if k == 9 { while_statement(&mut *c, depth); return; }
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
pub fn acyclic(c: &mut Compiler) -> () {
    if c.error != 0 { return; }
    let count = len(&*c.functions);
    let mut reach = bytes_new();
    if !grown(try_resize(&mut reach, count * count, 0u8)) { c.error = 68; return; }
    let mut e = 0;
    while e < len(&*c.edges) {
        let row = c.edges[e]; set_byte(&mut reach, row.caller * count + row.callee, 1); e = e + 1;
    }
    let mut k = 0;
    while k < count {
        let mut i = 0;
        while i < count {
            if byte_code(&reach, i * count + k) != 0 {
                let mut j = 0;
                while j < count {
                    if byte_code(&reach, k * count + j) != 0 { set_byte(&mut reach, i * count + j, 1); }
                    j = j + 1;
                }
            }
            i = i + 1;
        }
        k = k + 1;
    }
    let mut diagonal = 0;
    while diagonal < count {
        if byte_code(&reach, diagonal * count + diagonal) != 0 { fail(&mut *c); return; }
        diagonal = diagonal + 1;
    }
    return;
}
