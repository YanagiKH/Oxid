# Architecture

Oxid 0.9 uses a Rust frontend and interpreter. Packaging stores the parsed program as a serialized AST; an Oxid runtime is still needed to execute it.

## Language path

1. Preprocessing expands one-line macros and uses a deterministic cache.
2. The lexer and parser accept classical and concise syntax and attach file, line, and column ranges.
3. Module loading resolves imports relative to the importing file and through project dependency roots.
4. The interpreter executes source directly, or the compiler serializes the same program into OXBC 1.0.
5. The runtime decodes `.oxb`/`.oxa` artifacts, validates their version and checksum, and executes the retained AST.

## Runtime services

- arrays and deterministic records;
- bounded JSON parsing and stringification;
- lazy task values with observable state and memoized completion; joining multiple tasks is sequential;
- persistent nonblocking TCP listener/connection handles with bounded reads, writes, and HTTP parsing;
- files, environment, processes, and linked C/C++ functions;
- Python, Java, Go, C, and C++ bridge generation.

## Package path

`oxid.toml` declares local path or commit-pinned Git dependencies. The resolver installs Git checkouts below `.oxid/deps/`, walks nested manifests, detects dependency cycles, computes package-tree checksums, and writes deterministic `oxid.lock` version 1.

## Self-hosting boundary

`compiler/providers.toml` declares ownership without dispatching production
components. The binary OXBC writer is Rust code; the Oxid emitter is a separate
textual instruction demonstration. The legacy bootstrap commands verify AST
serialization round-trip and run that demonstration, not compiler self-rebuild.

See [the scoped implementation baseline](architecture/current-baseline.md),
[legacy characterization](../spec/legacy-0.9.md), and
[self-hosting requirements](SELF_HOSTING.md).
