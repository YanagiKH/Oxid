# Roadmap

## Oxid 0.9

- versioned OXBC 1.0 artifacts with serialized AST version 1, checksums, compatibility checks, and source ranges;
- record literals, property/index access, deterministic record storage, and bounded JSON parsing/stringification;
- reusable nonblocking TCP listener and connection handles with timeout-bounded HTTP helpers;
- source ranges preserved across imported modules and through compiled artifacts;
- deterministic `oxid.lock` version 1 with recursive path and commit-pinned HTTPS Git dependency resolution;
- locked, offline, frozen, update, fetch, install, list, add, and remove package workflows;
- a benchmark harness for cold start, parser throughput, packaging, and runtime operations;
- direct source interpretation alongside deterministic `.oxb` compilation and `.oxa` AST emission;
- an Oxid-authored normalized bytecode emitter and deterministic stage-0/stage-1/stage-2 artifact checks;
- a provider manifest that allows frontend components to move from stage-0 to Oxid incrementally;
- CI byte-comparison of compiler artifacts from Linux, Windows, macOS x86_64, and macOS arm64 before release packaging.

## Current self-hosting boundary

The bytecode emitter is Oxid-authored, while lexer, parser, diagnostics, and module providers still use stage-0. Bootstrap verifies a deterministic codec fixed point and executes the compiled Oxid compiler surface. This is an auditable migration boundary, not yet a fully independent Oxid compiler.

## Next

- replace the lexer, parser, diagnostics, and module providers one at a time, retaining deterministic parity gates;
- make the self-hosted compiler the default release path only after the remaining stage-0 providers have been replaced and independent compiler parity is demonstrated across release platforms;
- extend the cooperative task model into a scheduler suitable for large numbers of simultaneous network operations;
- publish repeatable benchmark history and regression thresholds based on representative workloads.

Oxid aims to improve development ergonomics and build latency, but the project does not claim a universal performance advantage over Rust. Use the benchmark harness with the intended workload and environment.
