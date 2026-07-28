# Roadmap

## Phase 0 - stabilize the bootstrap

- keep the Rust runtime as a temporary boot layer
- standardize parser and runtime error formatting
- move reusable compiler helpers into Oxid modules
- keep package and tool scripts source-driven

## Phase 1 - Oxid-first frontend helpers

- add an Oxid lexer module
- add an Oxid parser module
- add an Oxid diagnostics module
- add an Oxid module resolution module
- add a single prelude for common imports
- add example-driven frontend previews

## Phase 2 - frontend orchestration

- move parsing stage wiring into Oxid
- move source recovery logic into Oxid
- move module graph construction into Oxid
- expose a clearer pipeline for syntax, diagnostics, and loading

## Phase 3 - self-hosting scaffolding

- define a stable compiler data model in Oxid
- split parse, check, and emit stages
- add cache-aware rebuild metadata
- keep error formatting and notes readable in the terminal

## Phase 4 - bootstrap reduction

- compile Oxid source with more Oxid-written tooling
- keep Rust only as a bootstrap fallback
- expand syntax while preserving readability
- keep import and module resolution cheap to understand
