# Roadmap

## Phase 0

- remove Rust-specific execution dependence as far as the current project shape allows
- keep a compatibility fallback only where necessary

## Phase 1

- complete front-end modules in Oxid
- wire command orchestration to Oxid scripts
- standardize diagnostics and module resolution

## Phase 2

- move parser helpers and syntax validation into Oxid
- add recovery helpers and AST helpers
- expand syntax previews

## Phase 3

- move more compiler workflow and emission logic into Oxid
- keep Rust only as a bootstrap compatibility layer

## Phase 4

- keep the Rust layer minimal enough that day-to-day compiler work happens in Oxid
