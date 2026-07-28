# Architecture

Oxid uses a Rust bootstrap runtime with the following layers:

- lexer and parser
- interpreter and environment
- source preprocessing and macro expansion
- module loading and caching
- CLI, REPL, build, and project tooling
- native C and C++ interop

The project intentionally keeps a large Oxid-written surface area in:

- `stdlib/` for reusable language modules
- `examples/` for runnable demonstrations
- `tools/` for workflow previews
- `packages/` for end-user project layout examples
- `docs/` for project-oriented guidance

The Rust runtime stays focused on parsing, execution, caching, filesystem integration, and native interop, while the surrounding Oxid files demonstrate how the language is meant to be used.
