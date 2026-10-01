# What Oxid does

Oxid is a small language for scripts, automation, and cross-language tools. A single `oxid` executable can run source, create projects, manage dependencies, and package programs.

## Two ways to run

- `oxid run app.ox` parses and interprets source.
- `oxid compile app.ox -o app.oxb` packages its module graph as a serialized AST. `oxid run app.oxb` loads it into the same interpreter.

An `.oxb` file requires a compatible Oxid runtime. Compilation does not produce a native executable. Import ordering also differs in one known case; see [Modules](MODULES.md).

## Included in 0.9

The language has functions, loops, pipelines, arrays, records, JSON, and lazy tasks. Runtime services cover files, processes, TCP/HTTP helpers, and a small set of linked C/C++ functions. Process adapters connect to Python, Java, and Go; package commands resolve local paths and commit-pinned Git dependencies.

Oxid is experimental. The production parser and binary artifact writer are Rust code. The Oxid-authored emitter is a separate textual demonstration; self-hosting commands currently test serialization round-trips. Static ownership checking, native compilation, a concurrent scheduler, and native AI training remain future work.

Start with the [quickstart](QUICKSTART.md). For technical detail, read the [architecture](ARCHITECTURE.md), [current implementation](architecture/current-baseline.md), and [roadmap](ROADMAP.md).
