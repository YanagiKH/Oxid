# Current implementation and evidence boundaries

Source baseline: `7e681d25fa58828b817d66a8701a6913a7158932` (Oxid 0.9.0).
This records part of M0, the initial implementation inventory and specification work. M0 remains in progress.

## Production path

`src/cli.rs` includes `src/main.rs`. The latter contains the lexer, parser, AST,
interpreter, module loader, and CLI. Runtime services are split into `artifact`,
`data`, `network`, `packages`, and `benchmark` modules.

Source execution parses into the dynamic `Value` interpreter. Numbers are `f64`;
arrays and records have shared `Rc<RefCell<...>>` storage. No typed HIR, OIR/CFG,
ownership checker, or `.ox` native machine-code backend is present in this path.
The linked C/C++ helpers accelerate selected host operations; their existence
does not mean Oxid source is compiled to native machine code.

`compile_file` calls `compile_program` and then Rust's `artifact::write`.
The resulting OXBC 1.0 container holds serialized AST version 1 with source
ranges. The runtime decodes and interprets it. The FNV checksum detects selected
accidental changes; it is not a cryptographic integrity or provenance proof.

## Demonstrations and declaration-only boundaries

`stdlib/frontend/bytecode.ox` emits a textual instruction representation.
`compiler/main.ox` checks a fixed representative instruction sequence. This text
is different from the binary OXBC writer used by `oxid compile`. It is an emitter
demonstration, not a compiler for arbitrary supported Oxid source.

`compiler/providers.toml` declares component ownership. `verify_provider_manifest`
checks for required substrings, and `oxid frontend` reports declarations. The
manifest does not dispatch production compiler components, prove the emitter
was used, or enforce a no-fallback execution mode.

`bootstrap_project` encodes a parsed compiler demonstration, decodes/re-encodes it
twice, compares bytes, and interprets the demonstration. This is an artifact
serialization round-trip, not a compiler rebuilding itself. The existing
`bootstrap`, `self-compile`, `emit`, and `self-host` commands are retained for
compatibility, as are the `stage0`, `stage1`, `stage2`, and manifest fields.
These legacy names must not be interpreted as C1/C2/C3 self-compilation evidence.

`stdlib/mega.ox` consists of generated identity functions. It is synthetic source
coverage, not evidence of standard-library API breadth. Its location and public
names remain unchanged in this baseline to avoid breaking existing imports.
Preview/demo programs under `tools/`, `examples/`, and `packages/` demonstrate
selected workflows; folder presence is not domain production certification.

## Other limits confirmed in code

- Source executes imports at their top-level position; artifacts hoist imported module statements before importer-local statements. Side-effect ordering can therefore differ. See [Modules](../MODULES.md#initialization-order).
- Tasks are lazy and memoized; `join_all` executes them sequentially.
- `net_read` returns text from received bytes. There is no general binary stream
  contract or incremental UTF-8 decoder in that API.
- The package resolver keys dependencies by name, supports paths/pinned Git,
  and uses FNV package-tree checksums. This is not multi-version registry or
  cryptographic supply-chain verification.
- Benchmarks measure Oxid startup/parser/packaging/interpreter operations.
  They do not run equivalent Rust workloads or demonstrate Rust parity.
- CI declares Linux x86_64, Windows x86_64, macOS x86_64, and macOS ARM64 jobs.
  Linux ARM64 is not in the configured matrix. Configuration alone is not a
  record of a successful run on any of these targets.

## Evidence and next boundaries

See [the machine-readable inventory](../feature-status.json),
[status definitions](feature-status.md), and
[local baseline results](baseline-validation.md).

The baseline records selected legacy behavior and its implementation limits.
Full grammar/scope, static semantics and safety, threat model, compatibility and
performance corpora, native compilation, provider dispatch, true self-hosting,
and native AI training remain open work. The future static-core design must use
its own specification rather than treating the legacy interpreter as its oracle.
