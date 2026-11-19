use crate::model::Compiler;
use crate::model::Parameter;
use crate::model::Function;
use crate::model::SourceModule;
use crate::model::grown;
use crate::model::kind;
use crate::model::expect;
use crate::model::fail;
use crate::model::token;
use crate::model::word;
use crate::model::same_name;
use crate::model::function;
use crate::model::module_named;
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
fn collect_module(c: &mut Compiler, source_module: i32) -> () {
    let literal_bytes_0 = b"main";
    let input = c.sources[source_module];
    c.cursor = input.first_token;

    while kind(&*c) != 0 && c.error == 0 {
        if kind(&*c) == 18 {
            if source_module != 0 { fail(&mut *c); return; }
            expect(&mut *c, 18);
            let module_name = c.cursor;
            expect(&mut *c, 1);
            expect(&mut *c, 25);
            if c.error != 0 { return; }
            if module_named(&*c, module_name) >= 0 { fail(&mut *c); return; }
            if len(&*c.sources) >= 128 { c.error = 68; return; }
            crate::delivery::load_module(&mut *c, module_name);
            continue;
        }
        if len(&*c.functions) >= 128 { c.error = 68; return; }
        let public = kind(&*c) == 19;
        if public { expect(&mut *c, 19); }
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
    if !grown(try_push(&mut *c.sources, SourceModule { name: -1, first_token: 0 })) {
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
    let mut index = 0;
    while index < len(&*c.functions) {
        let f = c.functions[index];
        let mut parameter_index = f.params;
        while parameter_index < f.params + f.count {
            let p = c.params[parameter_index];
            if function(&*c, f.source_module, p.name) >= 0 { fail(&mut *c); return; }
            parameter_index = parameter_index + 1;
        }
        index = index + 1;
    }
    return;
}
