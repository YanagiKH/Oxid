# Self-hosting plan

Oxid does not become Rust-free in one step. The practical path is staged.

## What should move out of Rust first

- command orchestration
- source preprocessing
- module loading helpers
- diagnostics formatting
- parser stage helpers
- cache metadata handling

## Frontend split

- `stdlib/frontend/lexer.ox`
- `stdlib/frontend/parser.ox`
- `stdlib/frontend/diagnostics.ox`
- `stdlib/frontend/modules.ox`
- `stdlib/frontend/pipeline.ox`
- `stdlib/frontend/syntax.ox`

## Bootstrap sequence

1. keep the current Rust binary as the launcher
2. move frontend helper logic into Oxid modules
3. move error formatting and package utilities into Oxid
4. move parser-stage helpers into Oxid
5. move command dispatch into Oxid
6. replace bootstrap internals only after the Oxid path is complete

## What this pack focuses on

- short and readable helper modules
- clearer error reporting
- source-driven tool scripts
- copyable package templates
- a path for future self-hosting
