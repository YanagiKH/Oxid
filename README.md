<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

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
- `docs/` contains workflow, compiler, and diagnostics notes
- `tests/` contains smoke tests

## Recommended first run

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/stdlib_smoke.ox
cargo run -- run examples/self_host_preview.ox
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## Package-first workflow

1. Create a project with `oxid new`.
2. Import `stdlib/prelude.ox` in new scripts.
3. Keep reusable helpers in `stdlib/`.
4. Keep app code in `src/`.
5. Keep compiler and workflow previews in `tools/`.
6. Keep runnable examples in `examples/`.
7. Use `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc` before release.

Read:

- `docs/SELF_HOSTING.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/ROADMAP.md`
