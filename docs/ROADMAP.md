# Roadmap

Oxid aims to become a language for everyday native development: safe code,
predictable builds, useful libraries, and tools that work together. The current
release is experimental; version 1.0 is still in development.

## Available in the 0.9 release

- Run scripts with functions, loops, arrays, records, JSON, and lazy tasks.
- Work with files, processes, TCP/HTTP helpers, and other languages through
  process adapters.
- Create projects, format code, check syntax, and run tests.
- Use local and commit-pinned Git dependencies with lockfiles.
- Distribute `.oxb` script packages that run with a compatible Oxid runtime.

Follow the [quickstart](QUICKSTART.md) to try these features.

## Available in development source

The native project workflow adds:

- Explicit types and static ownership checks for the supported language subset.
- Move-only records, fixed scalar arrays and finite nested enum payloads.
- Byte fields and byte-array fields with checked conversion and borrowing.
- Growable byte buffers and partial binary input/output on Linux x86_64.
- Native executables on Linux x86_64 using LLVM/Clang/LLD 19.1.7.
- Development and optimized release builds with the same runtime checks.
- Local package dependencies, workspaces, and shared dependency locks.
- Project checks, tests, and builds that preserve the previous executable when
  a build fails.
- Optional native object caching and a command to diagnose the build environment.
- Portable source bundles containing local dependencies and a matching lock.
- Reproducible native builds when moving an unchanged source layout with the
  same compiler, toolchain, settings and environment.

These features are experimental and **not included in the published 0.9
binaries**. Programs currently have limits on execution steps and resource use.
See the [native project guide](TYPED_PROJECTS.md) and
[byte-count application](../apps/byte-count/README.md) for a working example.

## Planned for 1.0

| Area | Intended result |
|---|---|
| Language stability | A stable typed edition with documented semantics, compatibility rules, and migration guidance. |
| Memory and concurrency | General strings and growable containers, borrowing and cleanup, and safe concurrent programs. |
| Native builds | Windows, macOS, and Linux support on the planned x86_64 and ARM64 targets; debugging and reproducibility across supported platforms. |
| Runtime and services | Files, streams, processes, threads, asynchronous I/O, cancellation, HTTPS, and database applications. |
| Packages and builds | A package registry, multiple dependency versions, vendoring, and incremental builds across modules. |
| Developer tools | Editor integration, formatting, testing, debugging, documentation, and profiling using the same project settings. |
| Other languages | Native library integration with C and Rust, plus Python extensions with defined ownership and error handling. |
| Compiler | Build the Oxid compiler and standard library using Oxid. |
| Applications | Complete command-line tools, services, background workers, native libraries, and CPU model training. |
| Distribution | Versioned documentation and verifiable packages with reliable installation, updates, and rollback. |

The planned native targets are Linux x86_64 and ARM64, macOS x86_64 and ARM64,
and Windows x86_64. Their availability will be documented as each is implemented
and tested. Performance comparisons will use equivalent workloads and publish
the measured results.

The [technical release criteria](RELEASE_CRITERIA_1_0.md) define what must work
before version 1.0 can be released. The table above describes goals, not features
already available in a release.

## Later directions

GPU and distributed training, broader Rust and Python compatibility, WebAssembly,
and embedded targets are longer-term goals.
