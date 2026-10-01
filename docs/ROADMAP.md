# Roadmap

Oxid's long-term direction is a statically checked, native-compiled language with its own toolchain. The current 0.9 release is an experimental interpreter. This page separates the working baseline from the development ahead; it is not a release-date commitment.

## Available in 0.9

- Source interpretation and deterministic OXBC 1.0 serialized-AST artifacts
- Functions, loops, pipelines, records, JSON, and lazy task values
- Files, processes, TCP/HTTP helpers, and cross-language process adapters
- Local and commit-pinned Git dependencies with version 1 lockfiles
- Project scaffolding, checks, tests, formatting, and internal benchmarks

See the [implementation baseline](architecture/current-baseline.md) for limitations. The Oxid textual emitter and legacy bootstrap commands are demonstrations and serialization checks; they do not establish compiler self-hosting.

## Development sequence

| Stage | Work | Required evidence |
|---|---|---|
| M0: establish the baseline | Inventory features, characterize legacy behavior, write specifications and trust boundaries | Reproducible tests, scoped claims, reviewed contracts |
| Frontend and static core | Modular frontend, typed AST/HIR, analyzable IR, ownership and memory rules | Positive and compile-fail tests for the defined safe subset |
| Native compilation | First LLVM backend, native CLI, files, processes, and FFI contracts | Executed native programs on declared targets |
| Runtime and ecosystem | Concurrent scheduling, Web services, packages, debugging, and migration tools | Representative applications and failure/recovery tests |
| Compatibility and self-hosting | Scoped Rust migration, production provider dispatch, compiler rebuilds | Compatibility corpus, no hidden fallback, compiler fixed point, rebuild without Rust |
| Domain validation | Production pilots and broader platform support | Domain-specific correctness, security, deployment, and performance results |

M0 is in progress. The current inventory and selected legacy tests cover only part of that stage. New semantics require [RFCs](../rfcs/README.md); the legacy interpreter is not the specification for the future static core.

## Native AI work

Native AI support needs typed buffers and CPU tensors first, followed by automatic differentiation, optimizers, and complete training loops. GPU execution, tensor compilation, multi-device training, and resumable checkpoints add separate hardware and correctness requirements. Calling an external framework through a process adapter does not satisfy these milestones.

## How progress is reported

The [feature inventory](feature-status.json) links implementations, tests, evidence, and gaps. A feature becomes validated only for its declared scope and tested targets. Native compilation, self-hosting, Rust compatibility, production readiness, and native AI training each need their own evidence.

Performance claims require equivalent workloads and recorded environments. The current [benchmark harness](BENCHMARKS.md) measures Oxid itself; it does not establish a general speed advantage over Rust.
