[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)

# Oxid

Oxid is an experimental language prototype built around a Rust bootstrap runtime and a growing Oxid-written surface area.

The goal is not to stay as a thin wrapper. The project is organized so that everyday development work gradually moves into Oxid source files: standard library modules, examples, tooling scripts, package workflows, and project documentation.

## What is already available

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

## Language features

- `let` / `const`
- `print` / `if` / `while` / `fn` / `async fn` / `await` / `return` / `use`
- arrays, indexing, and indexed assignment
- `len` / `push` / `pop` / `range` / `str` / `sleep`
- `c_len` / `c_hash` / `cpp_len` / `cpp_hash`
- module cache, preprocess cache, and recursive loading
- single-line `macro` preprocessing expansion
- package manifest support (`oxid.toml` scripts / dependencies / features)

## Repository layout

- `src/` contains the Rust bootstrap runtime and entry script
- `stdlib/` contains Oxid standard library modules
- `examples/` contains runnable Oxid examples
- `tools/` contains Oxid tooling previews
- `packages/demo/` contains a user-facing package layout
- `docs/` contains workflow and architecture notes
- `tests/` contains smoke tests

## Recommended first run

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/stdlib_smoke.ox
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## Package-first workflow

The intended user experience is:

1. Create a project with `oxid new`.
2. Put reusable modules in `src/` and `stdlib/`.
3. Define scripts in `oxid.toml`.
4. Use `oxid script <name>` for repeatable tasks.
5. Keep examples and smoke tests alongside the package.
6. Use `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc` before release.

For a full walkthrough, read `docs/QUICKSTART.md` and `docs/PACKAGE_WORKFLOW.md`.
