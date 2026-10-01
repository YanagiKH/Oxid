# Oxid documentation

Start with a small program, then follow the guide for what you want to build. These pages describe the experimental 0.9 interpreter and tools; planned capabilities are listed separately in the roadmap.

## Start here

- [Installation](INSTALLATION.md): release binaries, source builds, and Docker
- [Quickstart](QUICKSTART.md): create, run, test, and package a project
- [Syntax](SYNTAX.md): functions, records, loops, and concise aliases
- [Runtime API](API.md): built-in values and functions
- [Commands](COMMANDS.md): CLI usage and important side effects

## Build something

- [Packages and lockfiles](PACKAGES.md) and [project workflow](PACKAGE_WORKFLOW.md)
- [Modules](MODULES.md) and [manifest scripts](SCRIPTS.md)
- [Cross-language integration](INTEROP.md) and [linked C/C++ helpers](FFI.md)
- [Local HTTP services and Discord handlers](WEB_AND_BOTS.md)
- [Tasks and I/O limits](ASYNC.md)

## Understand the implementation

- [Overview](OVERVIEW.md) and [architecture](ARCHITECTURE.md)
- [OXBC artifacts](COMPILER.md), [frontend](FRONTEND.md), and [diagnostics](DIAGNOSTICS.md)
- [Preprocessing macros](MACROS.md), [value sharing](LIFETIME.md), and [caches](CACHE.md)
- [Tooling](TOOLS.md) and [benchmarks](BENCHMARKS.md)
- [Legacy bootstrap commands](BOOTSTRAP.md), [command aliases](SELF_COMPILING.md), and [self-hosting status](SELF_HOSTING.md)

## Follow development

- [Roadmap](ROADMAP.md) and [contribution guide](../CONTRIBUTING.md)
- [Current implementation and limits](architecture/current-baseline.md)
- [Specification scope](../spec/README.md) and [selected legacy behavior](../spec/legacy-0.9.md)
- [Feature status definitions](architecture/feature-status.md) and [machine-readable inventory](feature-status.json)
- [Local baseline validation](architecture/baseline-validation.md) and [RFC process](../rfcs/README.md)
