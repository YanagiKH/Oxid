# Architecture

Oxid uses a Rust bootstrap runtime with the following layers:

- lexer and parser
- interpreter and environment
- source preprocessing and macro expansion
- module loading and caching
- CLI, REPL, build, and project tooling
- native C and C++ interop

The project keeps a large Oxid-written surface area in stdlib, tools, examples, and documentation-oriented source files.
