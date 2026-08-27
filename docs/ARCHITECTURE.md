# Architecture

Oxid 0.9 uses a stage-0 Rust bootstrap around a growing Oxid-authored compiler surface.

## Language path

1. Preprocessing expands one-line macros and uses a deterministic cache.
2. The lexer and parser accept classical and concise syntax and attach file, line, and column ranges.
3. Module loading resolves imports relative to the importing file and through project dependency roots.
4. The interpreter executes source directly, or the compiler serializes the same program into OXBC 1.0.
5. The runtime decodes `.oxb`/`.oxa` artifacts, validates their version and checksum, and executes the retained AST.

## Runtime services

- arrays and deterministic records;
- bounded JSON parsing and stringification;
- task values with observable state and memoized completion;
- persistent nonblocking TCP listener/connection handles with bounded reads, writes, and HTTP parsing;
- files, environment, processes, and linked C/C++ functions;
- Python, Java, Go, C, and C++ bridge generation.

## Package path

`oxid.toml` declares local path or commit-pinned Git dependencies. The resolver installs Git checkouts below `.oxid/deps/`, walks nested manifests, detects dependency cycles, computes package-tree checksums, and writes deterministic `oxid.lock` version 1.

## Self-hosting boundary

`compiler/providers.toml` selects the Oxid emitter while lexer, parser, diagnostics, and module handling remain stage-0. Bootstrap compiles and executes the Oxid compiler surface and verifies byte-identical stage artifacts. Provider replacement is incremental and parity-gated; the release compiler is not yet fully independent of stage-0.

