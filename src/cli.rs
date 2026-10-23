#[allow(clippy::items_after_test_module)]
mod runtime {
    include!("main.rs");

    pub fn entry(args: Vec<String>) {
        main(args);
    }
}

mod doctor_cli;
mod frontend;
mod program_arguments;
mod project_cli;
mod project_lock;
mod project_package;

fn main() {
    let mut invocation = match program_arguments::Invocation::parse(std::env::args_os().collect()) {
        Ok(invocation) => invocation,
        Err(message) => argument_error(message),
    };
    let _program_arguments = match invocation.enter() {
        Ok(scope) => scope,
        Err(message) => argument_error(message),
    };
    let mut args = std::mem::take(&mut invocation.compiler);
    if let Some(code) = project_package::dispatch(&args) {
        std::process::exit(code);
    }
    if let Some(code) = doctor_cli::dispatch(&args) {
        std::process::exit(code);
    }
    if let Some(code) = project_cli::dispatch(&mut args) {
        std::process::exit(code);
    }
    if let Some(code) = frontend::dispatch(&mut args) {
        std::process::exit(code);
    }
    match invocation.legacy(args) {
        Ok(args) => runtime::entry(args),
        Err(message) => argument_error(message),
    }
}

fn argument_error(message: &str) -> ! {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "error: {message}");
    std::process::exit(2);
}
