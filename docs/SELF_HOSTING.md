# Self-hosting plan

## Stage 1

- keep Rust only as a temporary compatibility fallback
- move command orchestration into Oxid scripts
- centralize diagnostics formatting
- centralize module resolution

## Stage 2

- move parser helpers into Oxid-owned modules
- move syntax preview and validation into Oxid-owned modules
- add structured front-end stages

## Stage 3

- move more compiler workflow code into Oxid
- reduce Rust-specific project logic
- keep native and bootstrap logic isolated

## Stage 4

- make Oxid the primary development language for the compiler toolchain
- keep Rust only as an optional compatibility backstop
