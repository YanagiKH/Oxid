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
use crate::model::function_visible;
use crate::model::child_module;
use crate::model::path_module;
use crate::model::array_type;
use crate::model::is_array;
use crate::model::is_slice;
use crate::model::slice_type;
use crate::model::is_value_scalar;
use crate::model::record;
use crate::model::record_type;
use crate::model::is_record;
use crate::model::record_field;
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
        expect(&mut *c, 51);
        let element = scalar_type(kind(&*c));
        if element == 0 { fail(&mut *c); return 0; }
        c.cursor = c.cursor + 1;
        expect(&mut *c, 52);
        return slice_type(element);
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
    if k == 1 {
        let index = record(&*c, source_module, c.cursor);
        if index >= 0 {
            c.cursor = c.cursor + 1;
            return record_type(index);
        }
    }
    fail(&mut *c); return 0;
}

fn collect_record(c: &mut Compiler, source_module: i32) -> () {
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
    expect(&mut *c, 54);
    let name = c.cursor;
    expect(&mut *c, 1); expect(&mut *c, 22);
    if record(&*c, source_module, name) >= 0 { fail(&mut *c); return; }
    let mut row = Record { name: name, source_module: source_module,
        first_field: len(&*c.fields), field_count: 0, start: start, end: 0,
        copy: copy_record, tagged: false };
    while kind(&*c) != 23 && c.error == 0 {
        if row.field_count >= 64 { c.error = 68; return; }
        if kind(&*c) == 19 { expect(&mut *c, 19); }
        let field_name = c.cursor;
        expect(&mut *c, 1); expect(&mut *c, 24);
        let field_type = ty(&mut *c, source_module);
        let allowed = is_value_scalar(field_type) || (!copy_record && is_array(field_type));
        if !allowed || record_field(&*c, row, field_name) >= 0 { fail(&mut *c); return; }
        let field = Field { name: field_name, ty: field_type };
        if !grown(try_push(&mut *c.fields, field)) { c.error = 68; return; }
        row.field_count = row.field_count + 1;
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    if row.field_count == 0 { fail(&mut *c); return; }
    row.end = c.cursor;
    if c.error == 0 && !grown(try_push(&mut *c.records, row)) { c.error = 68; }
    return;
}

fn collect_enum(c: &mut Compiler, source_module: i32) -> () {
    if len(&*c.records) >= 128 { c.error = 68; return; }
    let start = c.cursor;
    expect(&mut *c, 62);
    let name = c.cursor;
    expect(&mut *c, 1);
    expect(&mut *c, 22);
    if record(&*c, source_module, name) >= 0 { fail(&mut *c); return; }
    let mut row = Record { name: name, source_module: source_module,
        first_field: len(&*c.fields), field_count: 0, start: start, end: 0,
        copy: false, tagged: true };
    while kind(&*c) != 23 && c.error == 0 {
        if row.field_count >= 64 { c.error = 68; return; }
        let variant = c.cursor;
        expect(&mut *c, 1);
        if record_field(&*c, row, variant) >= 0 { fail(&mut *c); return; }
        let mut payload = 3;
        if kind(&*c) == 20 {
            expect(&mut *c, 20);
            payload = ty(&mut *c, source_module);
            expect(&mut *c, 21);
            if payload == 3 || is_slice(payload) { fail(&mut *c); return; }
            if is_record(payload) {
                let nested = c.records[crate::model::record_index(payload)];
                if nested.tagged { fail(&mut *c); return; }
            }
        }
        if c.error != 0 { return; }
        if !grown(try_push(&mut *c.fields, Field { name: variant, ty: payload })) { c.error = 68; return; }
        row.field_count = row.field_count + 1;
        if kind(&*c) != 23 { expect(&mut *c, 26); }
    }
    expect(&mut *c, 23);
    if row.field_count == 0 { fail(&mut *c); return; }
    row.end = c.cursor;
    if c.error == 0 && !grown(try_push(&mut *c.records, row)) { c.error = 68; }
    return;
}

fn discover_records(c: &mut Compiler, source_module: i32) -> () {
    let mut depth = 0;
    while kind(&*c) != 0 && c.error == 0 {
        let k = kind(&*c);
        if (k == 53 || k == 54) && depth == 0 { collect_record(&mut *c, source_module); continue; }
        if k == 62 && depth == 0 { collect_enum(&mut *c, source_module); continue; }
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

fn collect_import(c: &mut Compiler, source_module: i32) -> () {
    if len(&*c.imports) >= 4096 { c.error = 68; return; }
    expect(&mut *c, 46);
    let path_start = c.cursor + 2;
    let name = path_name(&mut *c);
    let mut alias = name;
    if kind(&*c) == 47 {
        expect(&mut *c, 47);
        alias = c.cursor;
        expect(&mut *c, 1);
    }
    expect(&mut *c, 25);
    if c.error != 0 { return; }
    let row = Import { scope: source_module, path_start: path_start, name: name, alias: alias, target: -1 };
    if !grown(try_push(&mut *c.imports, row)) { c.error = 68; }
    return;
}

fn resolve_imports(c: &mut Compiler) -> () {
    let mut index = 0;
    while index < len(&*c.imports) && c.error == 0 {
        let mut row = c.imports[index];
        c.cursor = row.alias;
        let target_module = path_module(&*c, row.path_start, row.name, row.scope);
        if target_module < 0 { fail(&mut *c); return; }
        let target = function(&*c, target_module, row.name);
        if !function_visible(&*c, row.scope, target) || function(&*c, row.scope, row.alias) >= 0 {
            fail(&mut *c);
            return;
        }
        let mut previous = 0;
        while previous < index {
            let old = c.imports[previous];
            if old.scope == row.scope && (old.target == target || same_name(&*c, old.alias, row.alias)) {
                fail(&mut *c);
                return;
            }
            previous = previous + 1;
        }
        row.target = target;
        c.imports[index] = row;
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
        if public && is_record(parameter_type) { fail(&mut *c); return; }
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
    if is_slice(result) || (public && is_record(result)) { fail(&mut *c); return; }
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
    }
    return;
}

fn collect_module(c: &mut Compiler, source_module: i32) -> () {
    let input = c.sources[source_module];
    c.cursor = input.first_token;
    let mut next_record = len(&*c.records);
    discover_records(&mut *c, source_module);
    c.cursor = input.first_token;

    while kind(&*c) != 0 && c.error == 0 {
        let declaration = kind(&*c);
        if declaration == 53 || declaration == 54 || declaration == 62 {
            if next_record >= len(&*c.records) { fail(&mut *c); return; }
            let row = c.records[next_record];
            if row.start != c.cursor { fail(&mut *c); return; }
            c.cursor = row.end;
            next_record = next_record + 1;
            continue;
        }
        if kind(&*c) == 46 { collect_import(&mut *c, source_module); continue; }
        let public = kind(&*c) == 19;
        if public { expect(&mut *c, 19); }
        if kind(&*c) == 18 {
            expect(&mut *c, 18);
            let module_name = c.cursor;
            expect(&mut *c, 1);
            expect(&mut *c, 25);
            if c.error != 0 { return; }
            if child_module(&*c, source_module, module_name, true) >= 0
                || record(&*c, source_module, module_name) >= 0 { fail(&mut *c); return; }
            if len(&*c.sources) >= 128 || input.depth >= 32 { c.error = 68; return; }
            crate::delivery::load_module(&mut *c, source_module, module_name, public);
            continue;
        }
        collect_function(&mut *c, source_module, public);
    }
    return;
}

pub fn collect(c: &mut Compiler) -> () {
    if !grown(try_push(&mut *c.sources, SourceModule { name: -1, first_token: 0, parent: -1, depth: 0, public: true })) {
        c.error = 68;
        return;
    }
    let mut source_module = 0;
    while source_module < len(&*c.sources) && c.error == 0 {
        collect_module(&mut *c, source_module);
        source_module = source_module + 1;
    }
    if c.error != 0 { return; }
    if c.main < 0 { fail(&mut *c); return; }
    resolve_imports(&mut *c);
    if c.error != 0 { return; }
    let mut index = 0;
    while index < len(&*c.functions) {
        let f = c.functions[index];
        let mut parameter_index = f.params;
        while parameter_index < f.params + f.count {
            let p = c.params[parameter_index];
            if function_binding(&*c, f.source_module, p.name) >= 0 { fail(&mut *c); return; }
            parameter_index = parameter_index + 1;
        }
        index = index + 1;
    }
    return;
}
