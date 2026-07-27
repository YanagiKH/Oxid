# Architecture

Oxid is currently a single-file Rust runtime, split into:

- lexer / parser
- interpreter / environment
- macro preprocessing
- native FFI layer
- CLI / REPL / watch / build
- manifest scripts and cache helpers

This layout makes it easier to split the parser, checker, codegen, and package manager into separate crates later.
