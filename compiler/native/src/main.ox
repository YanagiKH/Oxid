mod model;
mod delivery;
mod lexer;
mod headers;
mod emit;
mod parser;
use crate::delivery::Input;

fn main() -> i32 {
    let input = crate::delivery::load();
    match input {
        Input::Failed(status) => { return status; },
        Input::Loaded(source) => {
            let mut compiler = crate::model::new_compiler(source);
            crate::lexer::scan(&mut compiler);
            if compiler.error != 0 { return compiler.error; }
            crate::headers::collect(&mut compiler);
            if compiler.error != 0 { return compiler.error; }
            crate::emit::preamble(&mut compiler);
            crate::parser::bodies(&mut compiler);
            crate::parser::acyclic(&mut compiler);
            if compiler.error != 0 { return compiler.error; }
            crate::emit::wrapper(&mut compiler);
            if compiler.error != 0 { return compiler.error; }
            // Process entry mode returns only the exit status. In particular,
            // result-mode's automatic "0\n" must never trail the LLVM stream.
            return crate::delivery::write_all(&compiler.module);
        },
    }
}
