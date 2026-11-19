use crate::model::Compiler;
use crate::model::Value;
use crate::model::grown;
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
    let literal_bytes_0 = b"-";

    let mut digits = b"0000000000";
    let mut at = 10;
    let mut remaining = value;
    if remaining > 0 { remaining = 0 - remaining; }
    while remaining != 0 || at == 10 {
        at = at - 1;
        let digit = 0 - remaining % 10;
        let digit_code = 48 + digit;
        digits[at] = digit_code.to_u8_checked();
        remaining = remaining / 10;
    }
    if value < 0 { append(&mut *c, destination, &literal_bytes_0); }
    append(&mut *c, destination, &digits[at..10]);
    return;
}
pub fn write_type(c: &mut Compiler, destination: i32, ty: i32, result: bool) -> () {
    let literal_bytes_0 = b"i32";
    let literal_bytes_1 = b"void";
    let literal_bytes_2 = b"i1";

    if ty == 1 { append(&mut *c, destination, &literal_bytes_0); }
    else {
        if ty == 3 && result { append(&mut *c, destination, &literal_bytes_1); }
        else { append(&mut *c, destination, &literal_bytes_2); }
    }
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
    if id >= 4096 { c.error = 68; return 0; }
    c.next_slot = id + 1;
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
pub fn constant(c: &mut Compiler, ty: i32, value: i32) -> Value {
    let literal_bytes_0 = b"  %v";
    let literal_bytes_1 = b" = add i32 0, ";
    let literal_bytes_2 = b" = icmp ne i32 0, ";
    let literal_bytes_3 = b"\n";

    if ty == 3 { return Value { ty: 3, id: -1 }; }
    let id = fresh(&mut *c);
    append(&mut *c, 2, &literal_bytes_0); number(&mut *c, 2, id);
    if ty == 1 { append(&mut *c, 2, &literal_bytes_1); }
    else { append(&mut *c, 2, &literal_bytes_2); }
    number(&mut *c, 2, value); append(&mut *c, 2, &literal_bytes_3);
    return Value { ty: ty, id: id };
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
    let literal_bytes_2 = b"call i32 @ox_";
    let literal_bytes_3 = b"add";
    let literal_bytes_4 = b"sub";
    let literal_bytes_5 = b"mul";
    let literal_bytes_6 = b"div";
    let literal_bytes_7 = b"rem";
    let literal_bytes_8 = b"(i32 ";
    let literal_bytes_9 = b", i32 ";
    let literal_bytes_10 = b")\n";
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
        append(&mut *c, 2, &literal_bytes_2);
        if op == 29 { append(&mut *c, 2, &literal_bytes_3); }
        if op == 30 { append(&mut *c, 2, &literal_bytes_4); }
        if op == 31 { append(&mut *c, 2, &literal_bytes_5); }
        if op == 32 { append(&mut *c, 2, &literal_bytes_6); }
        if op == 33 { append(&mut *c, 2, &literal_bytes_7); }
        append(&mut *c, 2, &literal_bytes_8); operand(&mut *c, 2, left);
        append(&mut *c, 2, &literal_bytes_9); operand(&mut *c, 2, right); append(&mut *c, 2, &literal_bytes_10);
        return Value { ty: 1, id: id };
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
pub fn call(c: &mut Compiler, function: i32, args: &[Value], result: i32) -> Value {
    let literal_bytes_0 = b"  ";
    let literal_bytes_1 = b"%v";
    let literal_bytes_2 = b" = ";
    let literal_bytes_3 = b"call ";
    let literal_bytes_4 = b" @f";
    let literal_bytes_5 = b"(";
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
        if i != 0 { append(&mut *c, 2, &literal_bytes_6); }
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
pub fn finish_function(c: &mut Compiler, function: i32) -> () {
    let literal_bytes_0 = b"define internal ";
    let literal_bytes_1 = b" @f";
    let literal_bytes_2 = b"(";
    let literal_bytes_3 = b", ";
    let literal_bytes_4 = b" %a";
    let literal_bytes_5 = b") {\nentry:\n";
    let literal_bytes_6 = b"}\n";

    let f = c.functions[function];
    append(&mut *c, 0, &literal_bytes_0); write_type(&mut *c, 0, f.result, true);
    append(&mut *c, 0, &literal_bytes_1); number(&mut *c, 0, function); append(&mut *c, 0, &literal_bytes_2);
    let mut i = 0;
    while i < f.count {
        if i != 0 { append(&mut *c, 0, &literal_bytes_3); }
        let p = c.params[f.params + i];
        write_type(&mut *c, 0, p.ty, false); append(&mut *c, 0, &literal_bytes_4); number(&mut *c, 0, i);
        i = i + 1;
    }
    append(&mut *c, 0, &literal_bytes_5);
    let slots = take(&mut *c.slots);
    let body = take(&mut *c.body);
    append(&mut *c, 0, &slots); append(&mut *c, 0, &body); append(&mut *c, 0, &literal_bytes_6);
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
fn preamble_text_3_0(c: &mut Compiler) -> () {
    let text = b"declare {i32, i1} @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare {i32, i1} @llvm.ssub.with.overf";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_3_1(c: &mut Compiler) -> () {
    let text = b"low.i32(i32, i32)\ndeclare {i32, i1} @llvm.smul.with.overflow.i32(i32, i32)\n";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_4_0(c: &mut Compiler) -> () {
    let text = b"define internal i32 @ox_";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_5_0(c: &mut Compiler) -> () {
    let text = b"add";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_6_0(c: &mut Compiler) -> () {
    let text = b"sub";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_7_0(c: &mut Compiler) -> () {
    let text = b"mul";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_8_0(c: &mut Compiler) -> () {
    let text = b"(i32 %a, i32 %b) {\nentry:\n  %p = call {i32, i1} @llvm.s";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_9_0(c: &mut Compiler) -> () {
    let text = b".with.overflow.i32(i32 %a, i32 %b)\n  %bad = extractvalue {i32, i1} %p, 1\n  br i1 %bad, label %er";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_9_1(c: &mut Compiler) -> () {
    let text = b"ror, label %ok\nerror:\n  call void @__oxid_overflow(ptr @ov, i64 17)\n  unreachable\nok:\n  %v = ext";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_9_2(c: &mut Compiler) -> () {
    let text = b"ractvalue {i32, i1} %p, 0\n  ret i32 %v\n}\n";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_10_0(c: &mut Compiler) -> () {
    let text = b"div";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_11_0(c: &mut Compiler) -> () {
    let text = b"rem";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_12_0(c: &mut Compiler) -> () {
    let text = b"(i32 %a, i32 %b) {\nentry:\n  %zero = icmp eq i32 %b, 0\n  br i1 %zero, label %zero_error, label %c";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_12_1(c: &mut Compiler) -> () {
    let text = b"heck\nzero_error:\n  call void @__oxid_overflow(ptr @dz, i64 17)\n  unreachable\ncheck:\n  %min = icm";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_12_2(c: &mut Compiler) -> () {
    let text = b"p eq i32 %a, -2147483648\n  %negative = icmp eq i32 %b, -1\n  %bad = and i1 %min, %negative\n  br i";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_12_3(c: &mut Compiler) -> () {
    let text = b"1 %bad, label %overflow_error, label %ok\noverflow_error:\n  call void @__oxid_overflow(ptr @ov, i";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_12_4(c: &mut Compiler) -> () {
    let text = b"64 17)\n  unreachable\nok:\n  %v = ";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_13_0(c: &mut Compiler) -> () {
    let text = b"sdiv";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_14_0(c: &mut Compiler) -> () {
    let text = b"srem";
    append(&mut *c, 0, &text);
    return;
}
fn preamble_text_15_0(c: &mut Compiler) -> () {
    let text = b" i32 %a, %b\n  ret i32 %v\n}\n";
    append(&mut *c, 0, &text);
    return;
}
pub fn preamble(c: &mut Compiler) -> () {

    preamble_text_0_0(&mut *c);
    preamble_text_1_0(&mut *c); preamble_text_1_1(&mut *c);
    preamble_text_2_0(&mut *c); preamble_text_2_1(&mut *c);
    preamble_text_3_0(&mut *c); preamble_text_3_1(&mut *c);
    let mut op = 29;
    while op <= 31 {
        preamble_text_4_0(&mut *c);
        if op == 29 { preamble_text_5_0(&mut *c); }
        if op == 30 { preamble_text_6_0(&mut *c); }
        if op == 31 { preamble_text_7_0(&mut *c); }
        preamble_text_8_0(&mut *c);
        if op == 29 { preamble_text_5_0(&mut *c); }
        if op == 30 { preamble_text_6_0(&mut *c); }
        if op == 31 { preamble_text_7_0(&mut *c); }
        preamble_text_9_0(&mut *c); preamble_text_9_1(&mut *c); preamble_text_9_2(&mut *c);
        op = op + 1;
    }
    op = 32;
    while op <= 33 {
        preamble_text_4_0(&mut *c);
        if op == 32 { preamble_text_10_0(&mut *c); } else { preamble_text_11_0(&mut *c); }
        preamble_text_12_0(&mut *c); preamble_text_12_1(&mut *c); preamble_text_12_2(&mut *c); preamble_text_12_3(&mut *c); preamble_text_12_4(&mut *c);
        if op == 32 { preamble_text_13_0(&mut *c); } else { preamble_text_14_0(&mut *c); }
        preamble_text_15_0(&mut *c);
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
    let text = b"()\n";
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
fn wrapper_text_8_0(c: &mut Compiler) -> () {
    let text = b"  ret i32 %status\n}\n; Oxid scalar compiler end\n";
    append(&mut *c, 0, &text);
    return;
}
pub fn wrapper(c: &mut Compiler) -> () {

    let entry = c.main;
    let f = c.functions[entry];
    wrapper_text_0_0(&mut *c);
    if f.result != 3 { wrapper_text_1_0(&mut *c); }
    wrapper_text_2_0(&mut *c); write_type(&mut *c, 0, f.result, true);
    wrapper_text_3_0(&mut *c); number(&mut *c, 0, entry); wrapper_text_4_0(&mut *c);
    if f.result == 1 { wrapper_text_5_0(&mut *c); }
    if f.result == 2 { wrapper_text_6_0(&mut *c); }
    if f.result == 3 { wrapper_text_7_0(&mut *c); }
    wrapper_text_8_0(&mut *c);
    return;
}
