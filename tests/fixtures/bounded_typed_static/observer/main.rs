//! Narrow observation wrapper around unchanged canonical Oxid static frontend sources.
#![allow(dead_code)]

use std::io::{self, Read};

mod frontend;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut project_source = false;
    let mut resolve_only = false;
    for arg in args {
        match arg.as_str() {
            "--project-source" if !project_source => project_source = true,
            "--resolve-only" if !resolve_only => resolve_only = true,
            _ => {
                eprintln!("canonical static observer accepts only optional --project-source and --resolve-only");
                std::process::exit(2);
            }
        }
    }
    let mut input = Vec::new();
    if let Err(error) = io::stdin().take(129).read_to_end(&mut input) {
        eprintln!("canonical static observer stdin read failed: {error}");
        std::process::exit(2);
    }
    if input.len() > 128 || !input.is_ascii() {
        eprintln!("canonical static observer domain is ASCII input of at most 128 bytes");
        std::process::exit(2);
    }
    frontend::observe_static(String::from_utf8(input).expect("ASCII was checked"), project_source, resolve_only);
}
