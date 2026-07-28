# Self-hosting plan

Oxid cannot become fully Rust-free in one step. The realistic path is staged.

## What needs to move out of Rust

- command orchestration
- source preprocessing
- package workflow handling
- diagnostics formatting
- module resolution helpers
- cache metadata handling
- later, parsing and semantic checks

## Suggested file split

- `stdlib/` holds shared Oxid helpers
- `tools/` holds workflow and bootstrap commands
- `examples/` holds visible usage examples
- `packages/demo/` holds a copyable project template
- `docs/` explains the staged migration

## Bootstrap sequence

1. Keep the current Rust binary as the launcher.
2. Put the compiler workflow description into Oxid modules.
3. Move error formatting and package utilities into Oxid.
4. Move parsing helpers into Oxid.
5. Move command dispatch into Oxid.
6. Replace bootstrap internals only after the Oxid path is complete.

## What this pack focuses on now

- short and readable helper modules
- better default project layout
- clearer error reporting
- source-driven tool scripts
- a path for future self-hosting
