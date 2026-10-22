# What Oxid does

Oxid is a Rust-inspired language for scripts and command-line tools. Use the
`oxid` command to run programs, create projects, check code, and run tests.

## Run scripts with Oxid 0.9

The published 0.9 release runs `.ox` source files in an interpreter:

```sh
oxid run app.ox
```

Scripts support functions, loops, arrays, records, JSON, files, processes, and
TCP/HTTP helpers. Projects can use local or commit-pinned Git dependencies.
Process adapters let scripts call programs written in other languages.

You can also package a script and its imports as an `.oxb` file. It runs with a
compatible Oxid runtime. See [script packages](COMPILER.md) for the commands and
import limitations.

Start with the [quickstart](QUICKSTART.md), then browse the
[language syntax](SYNTAX.md) and [built-in functions](API.md).

## Build native programs with development source

The development version also supports typed projects with static type and
ownership checks, local dependencies, workspaces, and native executables.
These experimental features are **not included in the published 0.9 binaries**.

Native builds require Linux x86_64, LLVM/Clang/LLD 19.1.7, and the system C
development tools. The supported language and execution limits are described in
the [native project guide](TYPED_PROJECTS.md). A built executable runs without
the Oxid compiler or source tree on a system with compatible runtime libraries.

Use the [documentation index](README.md) to find a guide, or the
[roadmap](ROADMAP.md) to see what is available and what is planned.
