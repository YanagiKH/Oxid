#[allow(clippy::items_after_test_module)]
mod runtime {
    include!("main.rs");

    pub fn entry(args: Vec<String>) {
        main(args);
    }
}

mod frontend;
mod project_cli;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let executable = args.remove(0);
    if let Some(code) = project_cli::dispatch(&mut args) {
        std::process::exit(code);
    }
    if let Some(code) = frontend::dispatch(&mut args) {
        std::process::exit(code);
    }
    args.insert(0, executable);
    runtime::entry(args);
}
