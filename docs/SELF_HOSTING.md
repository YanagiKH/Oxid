# Self-hosting plan

Oxid is still bootstrapped by Rust, but the codebase now treats that as a temporary boundary instead of a permanent design.

## Stage 1: isolate Rust

- keep Rust only for the minimal bootstrap/runtime boundary
- move command orchestration into Oxid source files
- centralize diagnostics formatting in Oxid modules
- centralize module resolution in Oxid modules
- keep all preview logic in reusable Oxid helpers

## Stage 2: move compiler-facing previews into Oxid

- move parser helpers into Oxid-owned modules
- move syntax preview and validation into Oxid-owned modules
- add structured front-end stages
- add diagnostic categories and suggestion helpers
- add command catalog helpers for the CLI
- add compile and self-compile snapshots

## Stage 3: make Oxid the working language of the toolchain

- move more compiler workflow code into Oxid
- reduce Rust-specific project logic
- keep native and bootstrap logic isolated
- keep the Rust layer small and stable
- let Oxid define the visible workflow surface

## Stage 4: self-host by default

- make Oxid the primary development language for the compiler toolchain
- keep Rust only as an optional compatibility backstop
- preserve bootstrap scripts for recovery
- preserve a small native layer for platform-specific entry points

## Success criteria

- build and workflow helpers are written in Oxid where possible
- syntax and diagnostics are described by Oxid modules first
- examples, packages, and scripts are Oxid-native
- the bootstrap path becomes a thin compatibility layer only
- the self-compile path can describe itself from Oxid code