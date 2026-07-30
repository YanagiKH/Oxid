<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

# Oxid

Oxid is a derivative of Rust and features its own self-hosted language toolchain. Its goal is to create a language that is faster, more concise, and easier to read than Rust, with its own syntax, modules, command-line workflow, and diagnostic model.

The repository is organized so that the visible toolchain surface lives in Oxid source files first: bootstrap previews, compiler previews, standard library modules, examples, tooling scripts, package workflows, and project documentation.

## Current tool surface

Run these through manifest scripts while developing from the repository root:

- `oxid script bootstrap`: preview the bootstrap path
- `oxid script compile`: preview the compile path
- `oxid script frontend`: preview lex / parse / AST / recovery / module / syntax flow
- `oxid script diagnose`: preview source-linked diagnostics
- `oxid script lint`: preview style and structural rules
- `oxid script emit`: preview emission
- `oxid script module`: preview module resolution
- `oxid script syntax`: preview the shorter syntax forms
- `oxid script interop`: preview C/C++/Java/Python bridge coverage
- `oxid script bridge`: preview bidirectional bridge helpers
- `oxid script self-host`: summarize the migration path away from Rust
- `oxid run <file.ox>`: run a script
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

## Repository layout

- `src/` contains the Rust bootstrap runtime and entry script
- `stdlib/` contains Oxid standard library modules and tool workflows
- `examples/` contains runnable Oxid examples
- `tools/` contains Oxid workflow previews
- `packages/demo/` contains a user-facing package layout
- `docs/` contains workflow, compiler, syntax, diagnostics, and interop notes
- `tests/` contains smoke tests

## Recommended first run

```bash
oxid script bootstrap
oxid script compile
oxid script frontend
oxid script self-host
oxid run src/main.ox
oxid test
```

## Package-first workflow

1. Create a project with `oxid new`.
2. Import `stdlib/prelude.ox` in new scripts.
3. Keep reusable helpers in `stdlib/`.
4. Keep app code in `src/`.
5. Keep compiler and workflow previews in `tools/`.
6. Keep runnable examples in `examples/`.
7. Use `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc` before release.

## Design intent

Oxid should feel shorter than Rust for everyday work:

- fewer explicit scaffolding steps
- compact module and import forms
- lightweight command-driven workflows
- readable diagnostics with line, column, hint, and recovery context
- easy integration with C/C++, Java, and Python through Oxid-side bridge helpers
- reusable preview modules that can later become first-class compiler features

Read:

- `docs/SELF_HOSTING.md`
- `docs/COMPILER.md`
- `docs/FRONTEND.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/MODULES.md`
- `docs/INTEROP.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/COMMANDS.md`
- `docs/ROADMAP.md`
