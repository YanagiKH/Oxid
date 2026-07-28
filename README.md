# Oxid

Oxid is an experimental language runtime based on Rust + C/C++.
The bootstrap runtime is written in Rust, while the higher-level project structure, standard library, examples, and tooling previews are written in Oxid source files.

## What works today

- `oxid run <file.ox>`
- `oxid script <name> [args...]`
- `oxid check <file.ox>`
- `oxid repl`
- `oxid new <project-name>` / `oxid init <project-name>`
- `oxid add <name> <path-or-target>`
- `oxid watch <file.ox>`
- `oxid build`
- `oxid clean`
- `oxid fmt [path]`
- `oxid test`
- `oxid doctor`
- `oxid doc`

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


## Repository language weighting

The repository is configured so GitHub Linguist treats `*.ox` as Oxid and the Rust bootstrap files as generated. That makes the Oxid source tree the visible primary language for the project.
