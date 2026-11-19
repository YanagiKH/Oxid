use crate::model::Compiler;
use crate::model::Parameter;
use crate::model::Function;
use crate::model::Import;
use crate::model::SourceModule;
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
use alloc::vec::len;
use alloc::vec::try_push;

pub fn ty(c: &mut Compiler) -> i32 {
    let k = kind(&*c);
    if k == 14 { c.cursor = c.cursor + 1; return 1; }
    if k == 15 { c.cursor = c.cursor + 1; return 2; }
    if k == 16 { c.cursor = c.cursor + 1; return 4; }
    if k == 20 { expect(&mut *c, 20); expect(&mut *c, 21); return 3; }
    fail(&mut *c); return 0;
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

fn collect_module(c: &mut Compiler, source_module: i32) -> () {
    let literal_bytes_0 = b"main";
    let input = c.sources[source_module];
    c.cursor = input.first_token;

    while kind(&*c) != 0 && c.error == 0 {
        if kind(&*c) == 46 { collect_import(&mut *c, source_module); continue; }
        let public = kind(&*c) == 19;
        if public { expect(&mut *c, 19); }
        if kind(&*c) == 18 {
            expect(&mut *c, 18);
            let module_name = c.cursor;
            expect(&mut *c, 1);
            expect(&mut *c, 25);
            if c.error != 0 { return; }
            if child_module(&*c, source_module, module_name, true) >= 0 { fail(&mut *c); return; }
            if len(&*c.sources) >= 128 || input.depth >= 32 { c.error = 68; return; }
            crate::delivery::load_module(&mut *c, source_module, module_name, public);
            continue;
        }
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
            let parameter_type = ty(&mut *c);
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
        let result = ty(&mut *c);
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
        if source_module == 0 && word(&*c.source, name_token.start, name_token.end, &literal_bytes_0) {
            c.main = len(&*c.functions) - 1;
            if count != 0 { fail(&mut *c); return; }
        }
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
