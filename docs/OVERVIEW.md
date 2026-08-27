# Overview

Oxid 0.9 is a standalone language toolchain with two compatible workflows: interpret `.ox` source directly for fast iteration, or compile the same module graph into a deterministic, versioned `.oxb` artifact for distribution. The language accepts classical and concise spellings in one parser.

The runtime includes tasks, records and JSON, persistent nonblocking TCP adapters, files and processes, native C/C++ functions, and bridges for Python, Java, Go, C, and C++. The package workflow resolves local paths and commit-pinned Git dependencies into a deterministic lockfile.

The compiler migration is intentionally measurable. An Oxid-authored bytecode emitter is active behind a provider manifest, while lexer, parser, diagnostics, and modules remain in the stage-0 bootstrap. Deterministic bootstrap parity is implemented; complete independent self-hosting is not yet claimed.

Performance depends on workload and platform. `oxid bench` supplies repeatable measurements rather than a universal claim against another language.

