use crate::model::Compiler;
use crate::model::Parameter;
use crate::model::Function;
use crate::model::grown;
use crate::model::kind;
use crate::model::expect;
use crate::model::fail;
use crate::model::token;
use crate::model::word;
use crate::model::same_name;
use crate::model::function;
use alloc::vec::len;
use alloc::vec::try_push;

pub fn ty(c: &mut Compiler) -> i32 {
    let k = kind(&*c);
    if k == 14 { c.cursor = c.cursor + 1; return 1; }
    if k == 15 { c.cursor = c.cursor + 1; return 2; }
    if k == 20 { expect(&mut *c, 20); expect(&mut *c, 21); return 3; }
    fail(&mut *c); return 0;
}
pub fn collect(c: &mut Compiler) -> () {
    let literal_bytes_0 = b"main";

    while kind(&*c) != 0 && c.error == 0 {
        if len(&*c.functions) >= 128 { c.error = 68; return; }
        expect(&mut *c, 3);
        let name = c.cursor;
        expect(&mut *c, 1);
        if function(&*c, name) >= 0 { fail(&mut *c); return; }
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
        let row = Function { name: name, params: first, count: count, result: result, begin: begin, end: end };
        if !grown(try_push(&mut *c.functions, row)) { c.error = 68; return; }
        let name_token = token(&*c, name);
        if word(&*c.source, name_token.start, name_token.end, &literal_bytes_0) {
            c.main = len(&*c.functions) - 1;
            if count != 0 { fail(&mut *c); return; }
        }
    }
    if c.main < 0 { fail(&mut *c); return; }
    let mut parameter_index = 0;
    while parameter_index < len(&*c.params) {
        let p = c.params[parameter_index];
        if function(&*c, p.name) >= 0 { fail(&mut *c); return; }
        parameter_index = parameter_index + 1;
    }
    return;
}
