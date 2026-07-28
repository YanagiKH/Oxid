<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

# Oxid

Oxid is a language project that is being pushed toward a self-hosted, faster, and simpler developer experience.

The current repository still uses a Rust bootstrap runtime, but the project layout is now organized to move more work into Oxid source files, shared modules, package workflows, tooling scripts, and diagnostics helpers.

## What this pack adds

- a tighter standard-library front door through `stdlib/prelude.ox`
- better string helpers for common authoring tasks
- reusable diagnostic helpers for parser/runtime errors
- a self-hosting roadmap that explains how Rust is phased out
- package and script examples that are easier to copy into real projects
- example files that show how users should structure Oxid-first development

## Current commands already in the project

- `oxid run <file.ox>`
- `oxid script <name> [args...]`
- `oxid repl`
- `oxid check <file.ox>`
- `oxid new <project-name>` / `oxid init <project-name>`
- `oxid add <name> <path-or-target>`
- `oxid watch <file.ox>`
- `oxid build`
- `oxid clean`
- `oxid fmt [path]`
- `oxid test`
- `oxid doctor`
- `oxid doc`

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

## Self-hosting direction

The runtime is still bootstrapped by Rust today. The goal is to move parsing, diagnostics, command orchestration, package workflow, and later code generation into Oxid modules in stages.

Read:

- `docs/SELF_HOSTING.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/ROADMAP.md`
