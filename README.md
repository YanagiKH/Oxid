# Oxid

Oxid is an experimental language prototype based on Rust + C/C++.

Its goal is not to be a thin wrapper. It is to turn the language core, development workflow, macros, modules, native interoperability, and learning path into a runnable project.

## Implemented so far

- `oxid run <file.ox>`: run a script
- `oxid script <name> [args...]`: execute a manifest script
- `oxid repl`: interactive REPL
- `oxid check <file.ox>`: syntax check
- `oxid new <project-name>` / `oxid init <project-name>`: scaffold a project
- `oxid add <name> <path-or-target>`: add a dependency entry
- `oxid watch <file.ox>`: watch files and rerun
- `oxid build`: validate the project
- `oxid clean`: clear build cache
- `oxid fmt [path]`: format Oxid source files
- `oxid test`: run smoke tests and examples
- `oxid doctor`: verify project health
- `oxid doc`: generate API docs
- `let` / `const` / `print` / `if` / `while` / `fn` / `async fn` / `await` / `return` / `use`
- arrays, indexing, and indexed assignment
- `len` / `push` / `pop` / `range` / `str` / `sleep`
- `c_len` / `c_hash` / `cpp_len` / `cpp_hash` native interop
- module cache, preprocess cache, and recursive loading
- single-line `macro` preprocessing expansion
- package manifest support (`oxid.toml` scripts / dependencies / features)
- developer tooling commands (`fmt`, `test`, `doctor`, `doc`, `script`, `add`, `clean`)
- GitHub Actions scaffold

## Language features

- variables and constants
- functions and async functions
- arrays and indexing
- conditions and loops
- function calls and task-style awaiting
- source preprocessing macros
- module loading with search paths
- C and C++ interoperability

## Repository layout

- `src/` contains the Rust bootstrap runtime
- `native/` contains the C and C++ bridge code
- `stdlib/` contains Oxid standard library modules
- `examples/` contains runnable Oxid examples
- `tools/` contains Oxid tooling previews
- `docs/` contains English documentation
- `tests/` contains smoke tests

## Workflow

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/library.ox
cargo run -- script run
cargo run -- build
cargo run -- fmt examples/hello.ox
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## Project direction

The goal is to move more and more of the everyday language surface into Oxid source files:

- Oxid standard library modules
- Oxid examples
- Oxid tooling scripts
- Oxid package workflow previews

The Rust core remains the bootstrap layer for parsing, execution, caching, and native interop.
