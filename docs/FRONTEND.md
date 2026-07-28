# Front-end split

This repository is organized around Oxid-owned front-end modules.

## Layers

- lexer
- parser
- AST helpers
- recovery helpers
- diagnostics
- module resolution
- pipeline orchestration
- syntax previews
- emit previews
- lint previews

## Files

- `stdlib/frontend/lexer.ox`
- `stdlib/frontend/parser.ox`
- `stdlib/frontend/ast.ox`
- `stdlib/frontend/recovery.ox`
- `stdlib/frontend/diagnostics.ox`
- `stdlib/frontend/modules.ox`
- `stdlib/frontend/pipeline.ox`
- `stdlib/frontend/syntax.ox`
- `stdlib/frontend/emit.ox`
- `stdlib/frontend/lint.ox`

## Goal

The front-end should remain readable without Rust-specific project logic, and each stage should be small enough to move independently.
