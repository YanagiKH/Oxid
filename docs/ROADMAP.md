# Roadmap

## Phase 0 - stabilize the bootstrap

- keep the Rust runtime as a temporary boot layer
- fix parser and runtime error formatting
- add reusable Oxid modules for diagnostics, packaging, and workflow helpers
- make project scripts easier to copy

## Phase 1 - Oxid-first authoring

- add `stdlib/prelude.ox`
- move repeated workflow logic into Oxid modules
- expand example coverage
- add friendlier package templates
- standardize error messages with file, line, column, and hint fields

## Phase 2 - compiler self-hosting scaffolding

- move front-end helper logic into Oxid modules
- define a stable compiler data model in Oxid
- split parsing, checking, and emit stages
- add a clearer pipeline for syntax errors and recovery

## Phase 3 - actual self-hosted build path

- compile Oxid source with an Oxid-written toolchain
- keep the Rust runtime only as a bootstrap fallback
- add cache-aware rebuilds and artifact tracking
- make package and script commands source-driven

## Phase 4 - backend replacement

- use Oxid for the top-level compiler workflow
- phase out Rust-specific assumptions
- add richer diagnostics and lower-friction syntax
- keep the implementation small enough that project code stays readable
