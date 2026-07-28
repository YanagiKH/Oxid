# Self-hosting plan

## Stage 1

- keep the Rust bootstrap runtime
- move command orchestration into Oxid-facing scripts
- centralize diagnostics formatting
- centralize module resolution logic

## Stage 2

- move parser helpers into Oxid-owned modules
- move syntax preview / validation into Oxid-owned modules
- add structured front-end stages

## Stage 3

- move more of the compiler workflow into Oxid
- keep Rust as a temporary bootstrap only
- reduce the amount of Rust-specific project logic

## Stage 4

- make Oxid the primary development language for the compiler toolchain
- keep the Rust layer as a bootstrap compatibility fallback only
