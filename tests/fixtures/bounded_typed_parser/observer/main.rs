//! Narrow observation wrapper around unchanged canonical Oxid parser sources.
#![allow(dead_code)]

use std::io::{self, Read};

mod frontend;

fn main() {
    let mut input = Vec::new();
    if let Err(error) = io::stdin().take(129).read_to_end(&mut input) {
        eprintln!("canonical parser observer stdin read failed: {error}");
        std::process::exit(2);
    }
    if input.len() > 128 || !input.is_ascii() {
        eprintln!("canonical parser observer domain is ASCII input of at most 128 bytes");
        std::process::exit(2);
    }
    frontend::observe(String::from_utf8(input).expect("ASCII was checked"));
}
