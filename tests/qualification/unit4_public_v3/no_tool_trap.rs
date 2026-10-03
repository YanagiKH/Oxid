//! Qualification-only trap: no LLVM installation is needed for denial hosts.
use std::io::Write;
fn main() {
    let path = std::env::var_os("UNIT4_TOOL_LOG").expect("trap evidence path");
    let mut output = std::fs::OpenOptions::new().create(true).append(true).open(path)
        .expect("trap evidence open");
    output.write_all(b"{\"unexpected_native_tool\":true}\n").expect("trap evidence write");
    output.flush().expect("trap evidence flush");
    eprintln!("unexpected native tool invocation");
    std::process::exit(121);
}
