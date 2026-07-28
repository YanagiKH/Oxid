# Frontend

The frontend is organized as a visible pipeline rather than a hidden compiler monolith.

## Stages

1. lex
2. parse
3. ast
4. recover
5. diagnose
6. module
7. syntax
8. emit

## Goals

- keep the stages small
- make stage names explicit
- keep preview modules reusable from examples and tooling
- make error recovery and suggestions visible to users

## Entry helpers

- `frontend_bootstrap(project_name, entry_point)`
- `frontend_pipeline(source_name)`
- `frontend_compile_plan(project_name, version, entry_point)`

## Why this matters

A user should be able to see how a file moves through the toolchain, and the toolchain should be able to show useful stage-level messages without requiring Rust-specific knowledge.
