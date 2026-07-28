# Front-end split

This repository is moving toward a formal compiler front-end layout.

## Layers

- `lexer` turns source text into tokens
- `parser` turns tokens into an AST or syntax tree
- `diagnostics` turns errors into readable source-linked messages
- `modules` resolves imports, aliases, and module groups
- `pipeline` coordinates the front-end stages
- `syntax` documents the intended Oxid-first syntax direction

## Files

- `stdlib/frontend/lexer.ox`
- `stdlib/frontend/parser.ox`
- `stdlib/frontend/diagnostics.ox`
- `stdlib/frontend/modules.ox`
- `stdlib/frontend/pipeline.ox`
- `stdlib/frontend/syntax.ox`

## Goal

The long-term goal is to keep the top-level compiler workflow readable, Oxid-owned, and easy to extend without Rust-style boilerplate.
