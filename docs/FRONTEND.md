# Frontend layer

This document defines the Oxid-written frontend shape used by the pack.

## Suggested layers

- lexer
- parser
- diagnostics
- module resolution
- pipeline
- syntax planning

## Purpose of each layer

### Lexer
Turns source text into a token stream.

### Parser
Turns tokens into declarations, statements, and expressions.

### Diagnostics
Formats errors with file, line, column, snippet, and hint text.

### Module resolution
Turns a module import path into a resolved source path and cache key.

### Pipeline
Combines lexer, parser, diagnostics, and module resolution into one flow.

### Syntax planning
Tracks short forms and future language features separately from the runtime.

## Working rule

The frontend should stay simple enough that the compiler can be explained without Rust-specific knowledge.
