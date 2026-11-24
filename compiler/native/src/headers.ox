use crate::model::Compiler;
use crate::model::Parameter;
use crate::model::Function;
use crate::model::Import;
use crate::model::SourceModule;
use crate::model::Record;
use crate::model::Field;
use crate::model::grown;
use crate::model::kind;
use crate::model::expect;
use crate::model::fail;
use crate::model::token;
use crate::model::word;
use crate::model::same_name;
use crate::model::function;
use crate::model::function_binding;
use crate::model::argument_count_binding;
use crate::model::argument_bytes_binding;
use crate::model::argument_status_binding;
use crate::model::read_some_binding;
use crate::model::write_some_binding;
use crate::model::write_stderr_binding;
use crate::model::read_status_binding;
use crate::model::write_status_binding;
use crate::model::is_binary_io_binding;
use crate::model::byte_buffer_binding;
use crate::model::growth_status_binding;
use crate::model::byte_operation_binding;
use crate::model::is_byte_operation;
use crate::model::function_visible;
use crate::model::child_module;
use crate::model::path_module;
use crate::model::array_type;
use crate::model::is_array;
use crate::model::is_slice;
use crate::model::slice_type;
use crate::model::is_value_scalar;
use crate::model::record;
use crate::model::record_binding;
use crate::model::record_type;
use crate::model::is_record;
use crate::model::record_visible;
use crate::model::record_index;
use crate::model::record_reference;
use crate::model::is_record_reference;
use crate::model::record_value_type;
use crate::model::value_cells;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::try_push;

fn scalar_type(k: i32) -> i32 {
    if k == 14 { return 1; }
    if k == 15 { return 2; }
    if k == 16 { return 4; }
    if k == 48 { return 5; }
    if k == 58 { return 6; }
    if k == 60 { return 7; }
    return 0;
}

pub fn ty(c: &mut Compiler, source_module: i32) -> i32 {
    let k = kind(&*c);
    let scalar = scalar_type(k);
    if scalar != 0 { c.cursor = c.cursor + 1; return scalar; }
    if k == 20 { expect(&mut *c, 20); expect(&mut *c, 21); return 3; }
    if k == 65 {
        expect(&mut *c, 65);
        let exclusive = kind(&*c) == 5;
        if exclusive { expect(&mut *c, 5); }
        if kind(&*c) != 51 {
            let target = named_type(&mut *c, source_module);
            if !is_record(target) { return 0; }
            let row = c.records[record_index(target)];
            if row.tagged { fail(&mut *c); return 0; }
            return record_reference(target, exclusive);
        }
        expect(&mut *c, 51);
        let element = scalar_type(kind(&*c));
        if element == 0 { fail(&mut *c); return 0; }
        c.cursor = c.cursor + 1;
        expect(&mut *c, 52);
        return slice_type(element, exclusive);
    }
    if k == 51 {
        expect(&mut *c, 51);
        let element = scalar_type(kind(&*c));
        if element == 0 { fail(&mut *c); return 0; }
        c.cursor = c.cursor + 1;
        expect(&mut *c, 25);
        let length = token(&*c, c.cursor);
        expect(&mut *c, 2); expect(&mut *c, 52);
        if c.error != 0 { return 0; }
        if length.value < -1024i64 { c.error = 68; return 0; }
        let count = 0i64 - length.value;
        return array_type(element, count.to_i32_checked());
    }
    return named_type(&mut *c, source_module);
}

fn named_type(c: &mut Compiler, source_module: i32) -> i32 {
    let k = kind(&*c);
    if k == 44 {
        let first = c.cursor + 2;
        let name = path_name(&mut *c);
        if c.error != 0 { return 0; }
        let module = path_module(&*c, first, name, source_module);
        let index = record(&*c, module, name);
        if record_visible(&*c, source_module, index) { return record_type(index); }
    }
    if k == 1 {
        let index = record_binding(&*c, source_module, c.cursor);
        if index >= 0 {
            c.cursor = c.cursor + 1;
            return record_type(index);
        }
    }
    fail(&mut *c); return 0;
}

fn discover_nominal(c: &mut Compiler, source_module: i32) -> () {
    let derive = b"derive";
    let copy = b"Copy";
    if len(&*c.records) >= 128 { c.error = 68; return; }
    let start = c.cursor;
    let copy_record = kind(&*c) == 53;
    if copy_record {
        expect(&mut *c, 53); expect(&mut *c, 51);
        let attribute = token(&*c, c.cursor);
        expect(&mut *c, 1); expect(&mut *c, 20);
        let trait_name = token(&*c, c.cursor);
        expect(&mut *c, 1); expect(&mut *c, 21); expect(&mut *c, 52);
        if !word(&*c.source, attribute.start, attribute.end, &derive)
            || !word(&*c.source, trait_name.start, trait_name.end, &copy) {
            fail(&mut *c); return;
        }
    }
    let public = kind(&*c) == 19;
    if public { expect(&mut *c, 19); }
    let tagged = kind(&*c) == 62;
    if tagged {
        if copy_record { fail(&mut *c); return; }
        expect(&mut *c, 62);
    } else { expect(&mut *c, 54); }
    let name = c.cursor;
    expect(&mut *c, 1); expect(&mut *c, 22);
    if record(&*c, source_module, name) >= 0 || child_module(&*c, source_module, name, true) >= 0 {
        fail(&mut *c); return;
    }
    let body = c.cursor;
    let mut nesting = 1;
    while nesting > 0 && c.error == 0 {
        let current = kind(&*c);
        if current == 0 { fail(&mut *c); return; }
        if current == 22 { nesting = nesting + 1; }
        if current == 23 { nesting = nesting - 1; }
        if nesting > 64 { c.error = 68; return; }
        c.cursor = c.cursor + 1;
    }
    let row = Record { name: name, source_module: source_module, first_field: 0, field_count: 0,
        start: start, body: body, end: c.cursor, cells: 0, depth: 1, layout_state: 0,
        copy: copy_record, tagged: tagged, public: public, needs_drop: false };
    if c.error == 0 && !grown(try_push(&mut *c.records, row)) { c.error = 68; }
    return;
}

fn field_already_seen(c: &Compiler, fields: &Vec<Field>, name: i32) -> bool {
    for index in 0..len(&*fields) {
        let field = fields[index];
        if same_name(&*c, field.name, name) { return true; }
    }
    return false;
}

fn enum_payload(c: &mut Compiler, source_module: i32) -> i32 {
    if kind(&*c) != 20 { return 3; }
    expect(&mut *c, 20);
    let payload = ty(&mut *c, source_module);
    expect(&mut *c, 21);
    if payload == 3 || is_slice(payload) || is_record_reference(payload) { fail(&mut *c); }
    return payload;
}

fn field_dependency(c: &mut Compiler, index: i32, depth: i32) -> Record {
    let position = c.cursor;
    let caller_module = c.error_module;
    layout_nominal(&mut *c, index, depth);
    if c.error == 0 { c.cursor = position; c.error_module = caller_module; }
    return c.records[index];
}

fn layout_nominal(c: &mut Compiler, index: i32, depth: i32) -> () {
    let mut row = c.records[index];
    if row.layout_state == 2 { return; }
    if row.layout_state == 1 { fail(&mut *c); return; }
    if depth > 9 { c.error = 68; return; }
    row.layout_state = 1;
    c.records[index] = row;
    c.cursor = row.body;
    c.error_module = row.source_module;
    if row.tagged { row.cells = 1; }
    // A dependency can append its own fields while the current fields are staged.
    let mut fields = new::<Field>();
    while kind(&*c) != 23 && c.error == 0 {
        if row.field_count >= 64 { c.error = 68; return; }
        let mut field_public = row.tagged;
        if !row.tagged && kind(&*c) == 19 { expect(&mut *c, 19); field_public = true; }
        let field_name = c.cursor;
        expect(&mut *c, 1);
        if field_already_seen(&*c, &fields, field_name) { fail(&mut *c); return; }
        let mut field_type = 0;
        if row.tagged { field_type = enum_payload(&mut *c, row.source_module); }
        else { expect(&mut *c, 24); field_type = ty(&mut *c, row.source_module); }
        if c.error != 0 { return; }
        let mut allowed = (row.tagged && field_type == 3) || is_value_scalar(field_type)
            || (!row.copy && is_array(field_type));
        if is_record(field_type) {
            let child = record_index(field_type);
            let declared = c.records[child];
            allowed = !declared.tagged && (!row.copy || declared.copy)
                && (!row.public || !field_public || declared.public);
            if allowed {
                let component = field_dependency(&mut *c, child, depth + 1);
                if c.error != 0 { return; }
                row.needs_drop = row.needs_drop || component.needs_drop;
                if !row.tagged && component.depth >= row.depth { row.depth = component.depth + 1; }
            }
        }
        if !allowed { fail(&mut *c); return; }
        if !grown(try_push(&mut fields, Field { name: field_name, ty: field_type, public: field_public })) {
            c.error = 68; return;
        }
        row.field_count = row.field_count + 1;
        // Rounding individual fields to eight-byte cells includes padding.
        row.cells = row.cells + value_cells(&*c, field_type);
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    if c.error != 0 { return; }
    if row.tagged && row.field_count == 0 { fail(&mut *c); return; }
    // A zero-sized value still has a distinct local slot identifier.
    if row.field_count == 0 { row.cells = 1; }
    if row.depth > 8 { c.error = 68; return; }
    row.first_field = len(&*c.fields);
    for field_index in 0..row.field_count {
        if !grown(try_push(&mut *c.fields, fields[field_index])) { c.error = 68; return; }
    }
    row.layout_state = 2;
    c.records[index] = row;
    return;
}

fn discover_records(c: &mut Compiler, source_module: i32) -> () {
    let mut depth = 0;
    while kind(&*c) != 0 && c.error == 0 {
        let k = kind(&*c);
        let next = token(&*c, c.cursor + 1);
        if (k == 53 || k == 54 || k == 62 || (k == 19 && (next.kind == 54 || next.kind == 62))) && depth == 0 {
            discover_nominal(&mut *c, source_module); continue;
        }
        if k == 22 { depth = depth + 1; }
        if k == 23 { depth = depth - 1; }
        c.cursor = c.cursor + 1;
    }
    return;
}
pub fn path_name(c: &mut Compiler) -> i32 {
    expect(&mut *c, 44);
    expect(&mut *c, 45);
    let mut name = c.cursor;
    expect(&mut *c, 1);
    while c.error == 0 && kind(&*c) == 45 {
        expect(&mut *c, 45);
        name = c.cursor;
        expect(&mut *c, 1);
    }
    return name;
}

fn expect_name(c: &mut Compiler, spelling: &[u8]) -> () {
    let part = token(&*c, c.cursor);
    if part.kind != 1 || !word(&*c.source, part.start, part.end, &*spelling) {
        fail(&mut *c); return;
    }
    expect(&mut *c, 1);
    return;
}

fn byte_import(c: &mut Compiler) -> i32 {
    let namespace = b"bytes";
    expect_name(&mut *c, &namespace); expect(&mut *c, 45);
    let item = token(&*c, c.cursor);
    expect(&mut *c, 1);
    let type_name = b"ByteBuf";
    let growth_name = b"GrowStatus";
    let create = b"new";
    let length = b"len";
    let capacity = b"capacity";
    let reserve = b"try_reserve";
    let push = b"try_push";
    let extend = b"try_extend";
    let resize = b"try_resize";
    let shrink = b"truncate";
    let transfer = b"take";
    if word(&*c.source, item.start, item.end, &type_name) { return byte_buffer_binding(); }
    if word(&*c.source, item.start, item.end, &growth_name) { return growth_status_binding(); }
    if word(&*c.source, item.start, item.end, &create) { return byte_operation_binding(0); }
    if word(&*c.source, item.start, item.end, &length) { return byte_operation_binding(1); }
    if word(&*c.source, item.start, item.end, &capacity) { return byte_operation_binding(2); }
    if word(&*c.source, item.start, item.end, &reserve) { return byte_operation_binding(3); }
    if word(&*c.source, item.start, item.end, &push) { return byte_operation_binding(4); }
    if word(&*c.source, item.start, item.end, &extend) { return byte_operation_binding(5); }
    if word(&*c.source, item.start, item.end, &resize) { return byte_operation_binding(6); }
    if word(&*c.source, item.start, item.end, &shrink) { return byte_operation_binding(7); }
    if word(&*c.source, item.start, item.end, &transfer) { return byte_operation_binding(10); }
    fail(&mut *c); return -1;
}

fn standard_import(c: &mut Compiler) -> i32 {
    let allocation = b"alloc";
    let root = token(&*c, c.cursor);
    if word(&*c.source, root.start, root.end, &allocation) {
        expect(&mut *c, 1); expect(&mut *c, 45);
        return byte_import(&mut *c);
    }
    let std_name = b"std";
    let env_name = b"env";
    let io_name = b"io";
    expect_name(&mut *c, &std_name); expect(&mut *c, 45);
    let namespace = token(&*c, c.cursor);
    expect(&mut *c, 1); expect(&mut *c, 45);
    let item = token(&*c, c.cursor);
    expect(&mut *c, 1);
    if word(&*c.source, namespace.start, namespace.end, &env_name) {
        let count_name = b"arg_count";
        let bytes_name = b"arg_bytes";
        let status_name = b"ArgStatus";
        if word(&*c.source, item.start, item.end, &count_name) { return argument_count_binding(); }
        if word(&*c.source, item.start, item.end, &bytes_name) { return argument_bytes_binding(); }
        if word(&*c.source, item.start, item.end, &status_name) { return argument_status_binding(); }
    }
    if word(&*c.source, namespace.start, namespace.end, &io_name) {
        let read_name = b"read_some";
        let write_name = b"write_some";
        let stderr_name = b"write_stderr";
        let read_type = b"ReadSomeStatus";
        let write_type = b"WriteSomeStatus";
        if word(&*c.source, item.start, item.end, &read_name) { return read_some_binding(); }
        if word(&*c.source, item.start, item.end, &write_name) { return write_some_binding(); }
        if word(&*c.source, item.start, item.end, &stderr_name) { return write_stderr_binding(); }
        if word(&*c.source, item.start, item.end, &read_type) { return read_status_binding(); }
        if word(&*c.source, item.start, item.end, &write_type) { return write_status_binding(); }
    }
    fail(&mut *c); return -1;
}

fn collect_import(c: &mut Compiler, source_module: i32, save: bool) -> () {
    if save && len(&*c.imports) >= 4096 { c.error = 68; return; }
    expect(&mut *c, 46);
    let mut path_start = -1;
    let mut name = 0;
    if kind(&*c) == 44 {
        path_start = c.cursor + 2;
        name = path_name(&mut *c);
    } else { path_start = standard_import(&mut *c); name = c.cursor - 1; }
    let mut alias = name;
    if kind(&*c) == 47 {
        expect(&mut *c, 47);
        alias = c.cursor;
        expect(&mut *c, 1);
    }
    expect(&mut *c, 25);
    if c.error != 0 || !save { return; }
    let row = Import { scope: source_module, path_start: path_start, name: name,
        alias: alias, target: -1, record_target: -1 };
    if !grown(try_push(&mut *c.imports, row)) { c.error = 68; }
    return;
}

fn status_field(c: &mut Compiler, name: i32, payload: i32) -> bool {
    if grown(try_push(&mut *c.fields, Field { name: name, ty: payload, public: true })) { return true; }
    c.error = 68;
    return false;
}

fn register_standard_status(c: &mut Compiler, binding: i32) -> i32 {
    let index = len(&*c.records);
    if index >= 128 { c.error = 68; return -1; }
    let first = len(&*c.fields);
    let mut count = 2;
    if binding == growth_status_binding() {
        count = 4;
        if !status_field(&mut *c, -9, 3) || !status_field(&mut *c, -10, 3)
            || !status_field(&mut *c, -11, 3) || !status_field(&mut *c, -12, 3) { return -1; }
    } else if binding == argument_status_binding() {
        count = 4;
        if !status_field(&mut *c, -5, 1) || !status_field(&mut *c, -6, 1)
            || !status_field(&mut *c, -7, 3) || !status_field(&mut *c, -8, 3) { return -1; }
    } else if binding == read_status_binding() {
        count = 3;
        if !status_field(&mut *c, -1, 1) || !status_field(&mut *c, -2, 3)
            || !status_field(&mut *c, -3, 1) { return -1; }
    } else {
        if !status_field(&mut *c, -4, 1) || !status_field(&mut *c, -3, 1) { return -1; }
    }
    let row = Record { name: binding, source_module: -1, first_field: first, field_count: count,
        start: 0, body: 0, end: 0, cells: count + 1, depth: 1, layout_state: 2,
        copy: false, tagged: true, public: true, needs_drop: false };
    if !grown(try_push(&mut *c.records, row)) { c.error = 68; return -1; }
    return index;
}

fn register_byte_buffer(c: &mut Compiler) -> i32 {
    let index = len(&*c.records);
    if index >= 128 { c.error = 68; return -1; }
    let row = Record { name: byte_buffer_binding(), source_module: -1, first_field: 0, field_count: 0,
        start: 0, body: 0, end: 0, cells: 2, depth: 1, layout_state: 2,
        copy: false, tagged: false, public: true, needs_drop: true };
    if !grown(try_push(&mut *c.records, row)) { c.error = 68; return -1; }
    return index;
}

fn register_standard_types(c: &mut Compiler) -> () {
    let mut read = false;
    let mut write = false;
    let mut arguments = false;
    let mut buffers = false;
    let mut growth = false;
    for index in 0..len(&*c.imports) {
        let row = c.imports[index];
        let binding = row.path_start;
        read = read || binding == read_some_binding() || binding == read_status_binding();
        write = write || binding == write_some_binding() || binding == write_stderr_binding()
            || binding == write_status_binding();
        arguments = arguments || binding == argument_bytes_binding() || binding == argument_status_binding();
        buffers = buffers || binding == byte_buffer_binding() || is_byte_operation(binding);
        growth = growth || binding == growth_status_binding()
            || (binding >= byte_operation_binding(6) && binding <= byte_operation_binding(3));
    }
    if read { c.read_status = register_standard_status(&mut *c, read_status_binding()); }
    if c.error == 0 && write { c.write_status = register_standard_status(&mut *c, write_status_binding()); }
    if c.error == 0 && arguments { c.argument_status = register_standard_status(&mut *c, argument_status_binding()); }
    if c.error == 0 && buffers { c.byte_buffer = register_byte_buffer(&mut *c); }
    if c.error == 0 && growth { c.growth_status = register_standard_status(&mut *c, growth_status_binding()); }
    return;
}

fn resolve_record_imports(c: &mut Compiler) -> () {
    for index in 0..len(&*c.imports) {
        let mut row = c.imports[index];
        let mut target = -1;
        if row.path_start == read_status_binding() { target = c.read_status; }
        else if row.path_start == write_status_binding() { target = c.write_status; }
        else if row.path_start == argument_status_binding() { target = c.argument_status; }
        else if row.path_start == byte_buffer_binding() { target = c.byte_buffer; }
        else if row.path_start == growth_status_binding() { target = c.growth_status; }
        else if row.path_start >= 0 {
            let module = path_module(&*c, row.path_start, row.name, row.scope);
            target = record(&*c, module, row.name);
        }
        if !record_visible(&*c, row.scope, target) { continue; }
        c.error_module = row.scope;
        c.cursor = row.alias;
        if record_binding(&*c, row.scope, row.alias) >= 0 { fail(&mut *c); return; }
        for previous in 0..index {
            let old = c.imports[previous];
            if old.scope == row.scope && old.record_target == target { fail(&mut *c); return; }
        }
        row.record_target = target;
        c.imports[index] = row;
    }
    return;
}

fn resolve_imports(c: &mut Compiler) -> () {
    let mut index = 0;
    while index < len(&*c.imports) && c.error == 0 {
        let mut row = c.imports[index];
        c.error_module = row.scope;
        c.cursor = row.alias;
        let mut target = row.path_start;
        if target < 0 && row.record_target >= 0 { index = index + 1; continue; }
        if row.path_start >= 0 {
            let target_module = path_module(&*c, row.path_start, row.name, row.scope);
            if target_module < 0 { fail(&mut *c); return; }
            target = function(&*c, target_module, row.name);
            if !function_visible(&*c, row.scope, target) {
                if row.record_target >= 0 { index = index + 1; continue; }
                fail(&mut *c); return;
            }
        }
        if function(&*c, row.scope, row.alias) >= 0 {
            fail(&mut *c);
            return;
        }
        let mut previous = 0;
        while previous < index {
            let old = c.imports[previous];
            if old.scope == row.scope && old.target != -1
                && (old.target == target || same_name(&*c, old.alias, row.alias)) {
                fail(&mut *c);
                return;
            }
            previous = previous + 1;
        }
        row.target = target;
        c.imports[index] = row;
        if target == argument_count_binding() { c.uses_argument_count = true; }
        if target == argument_bytes_binding() {
            c.uses_argument_count = true;
            c.uses_argument_bytes = true;
        }
        if is_binary_io_binding(target) {
            if !c.process_entry { fail(&mut *c); return; }
            c.uses_binary_io = true;
        }
        index = index + 1;
    }
    return;
}

fn collect_function(c: &mut Compiler, source_module: i32, public: bool) -> () {
    let entry_name = b"main";
    if len(&*c.functions) >= 128 { c.error = 68; return; }
    expect(&mut *c, 3);
    let name = c.cursor;
    expect(&mut *c, 1);
    if function(&*c, source_module, name) >= 0 { fail(&mut *c); return; }
    expect(&mut *c, 20);
    let first = len(&*c.params);
    let mut count = 0;
    while kind(&*c) != 21 && c.error == 0 {
        if count >= 16 { c.error = 68; return; }
        let parameter_name = c.cursor;
        expect(&mut *c, 1);
        expect(&mut *c, 24);
        let parameter_type = ty(&mut *c, source_module);
        let exposed_type = record_value_type(parameter_type);
        if public && is_record(exposed_type) {
            let selected = c.records[record_index(exposed_type)];
            if !selected.public { fail(&mut *c); return; }
        }
        let mut i = first;
        while i < first + count {
            let previous = c.params[i];
            if same_name(&*c, previous.name, parameter_name) { fail(&mut *c); return; }
            i = i + 1;
        }
        if !grown(try_push(&mut *c.params, Parameter { name: parameter_name, ty: parameter_type })) {
            c.error = 68; return;
        }
        count = count + 1;
        if kind(&*c) != 21 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 21);
    expect(&mut *c, 27);
    let result = ty(&mut *c, source_module);
    if is_slice(result) || is_record_reference(result) { fail(&mut *c); return; }
    if public && is_record(result) {
        let selected = c.records[record_index(result)];
        if !selected.public { fail(&mut *c); return; }
    }
    let begin = c.cursor;
    expect(&mut *c, 22);
    let mut depth = 1;
    while depth > 0 && c.error == 0 {
        let k = kind(&*c);
        if k == 0 { fail(&mut *c); return; }
        if k == 22 { depth = depth + 1; }
        if k == 23 { depth = depth - 1; }
        if depth > 64 { c.error = 68; return; }
        c.cursor = c.cursor + 1;
    }
    let end = c.cursor;
    let row = Function { name: name, params: first, count: count, result: result,
        begin: begin, end: end, source_module: source_module, public: public };
    if !grown(try_push(&mut *c.functions, row)) { c.error = 68; return; }
    let name_token = token(&*c, name);
    if source_module == 0 && word(&*c.source, name_token.start, name_token.end, &entry_name) {
        c.main = len(&*c.functions) - 1;
        if count != 0 || is_array(result) || is_record(result) { fail(&mut *c); return; }
        if c.process_entry && result != 1 { c.cursor = name; fail(&mut *c); return; }
    }
    return;
}

fn module_name(c: &mut Compiler) -> i32 {
    expect(&mut *c, 18);
    let name = c.cursor;
    expect(&mut *c, 1);
    expect(&mut *c, 25);
    return name;
}

fn discover_children(c: &mut Compiler, source_module: i32) -> () {
    let input = c.sources[source_module];
    c.cursor = input.first_token;
    let mut depth = 0;
    while kind(&*c) != 0 && c.error == 0 {
        let k = kind(&*c);
        let next = token(&*c, c.cursor + 1);
        let public = k == 19 && next.kind == 18;
        if depth == 0 && k == 46 { collect_import(&mut *c, source_module, true); continue; }
        if depth == 0 && (k == 18 || public) {
            if public { expect(&mut *c, 19); }
            let name = module_name(&mut *c);
            if c.error != 0 { return; }
            if child_module(&*c, source_module, name, true) >= 0 {
                c.cursor = name; fail(&mut *c); return;
            }
            if len(&*c.sources) >= 128 || input.depth >= 32 { c.error = 68; return; }
            crate::delivery::load_module(&mut *c, source_module, name, public);
            continue;
        }
        if k == 22 { depth = depth + 1; }
        if k == 23 { depth = depth - 1; }
        c.cursor = c.cursor + 1;
    }
    return;
}

fn collect_module(c: &mut Compiler, source_module: i32) -> () {
    let input = c.sources[source_module];
    c.cursor = input.first_token;
    let mut next_record = 0;
    while next_record < len(&*c.records) {
        let row = c.records[next_record];
        if row.source_module >= source_module { break; }
        next_record = next_record + 1;
    }

    while kind(&*c) != 0 && c.error == 0 {
        let declaration = kind(&*c);
        let next = token(&*c, c.cursor + 1);
        if declaration == 53 || declaration == 54 || declaration == 62
            || (declaration == 19 && (next.kind == 54 || next.kind == 62)) {
            if next_record >= len(&*c.records) { fail(&mut *c); return; }
            let row = c.records[next_record];
            if row.start != c.cursor { fail(&mut *c); return; }
            c.cursor = row.end;
            next_record = next_record + 1;
            continue;
        }
        if kind(&*c) == 46 { collect_import(&mut *c, source_module, false); continue; }
        let public = kind(&*c) == 19;
        if public { expect(&mut *c, 19); }
        if kind(&*c) == 18 {
            module_name(&mut *c);
            continue;
        }
        collect_function(&mut *c, source_module, public);
    }
    return;
}

pub fn collect(c: &mut Compiler) -> () {
    if !grown(try_push(&mut *c.sources, SourceModule { name: -1, first_token: 0, first_byte: 0, parent: -1, depth: 0, public: true })) {
        c.error = 68;
        return;
    }
    let mut source_module = 0;
    while source_module < len(&*c.sources) && c.error == 0 {
        c.error_module = source_module;
        discover_children(&mut *c, source_module);
        source_module = source_module + 1;
    }
    source_module = 0;
    while source_module < len(&*c.sources) && c.error == 0 {
        c.error_module = source_module;
        let input = c.sources[source_module];
        c.cursor = input.first_token;
        discover_records(&mut *c, source_module);
        source_module = source_module + 1;
    }
    if c.error != 0 { return; }
    register_standard_types(&mut *c);
    if c.error != 0 { return; }
    resolve_record_imports(&mut *c);
    for nominal_index in 0..len(&*c.records) {
        if c.error != 0 { return; }
        layout_nominal(&mut *c, nominal_index, 1);
    }
    source_module = 0;
    while source_module < len(&*c.sources) && c.error == 0 {
        c.error_module = source_module;
        collect_module(&mut *c, source_module);
        source_module = source_module + 1;
    }
    if c.error != 0 { return; }
    if c.main < 0 { c.error_module = 0; c.cursor = 0; fail(&mut *c); return; }
    resolve_imports(&mut *c);
    if c.error != 0 { return; }
    let mut index = 0;
    while index < len(&*c.functions) {
        let f = c.functions[index];
        let mut parameter_index = f.params;
        while parameter_index < f.params + f.count {
            let p = c.params[parameter_index];
            if function_binding(&*c, f.source_module, p.name) != -1 {
                c.error_module = f.source_module; c.cursor = p.name; fail(&mut *c); return;
            }
            parameter_index = parameter_index + 1;
        }
        index = index + 1;
    }
    return;
}
