# Current implementation and evidence boundaries

Source baseline: `7e681d25fa58828b817d66a8701a6913a7158932` (Oxid 0.9.0).
This records part of M0, the initial implementation inventory and specification work. M0 remains in progress.

## Production path

`src/cli.rs` includes the legacy runtime in `src/main.rs`; legacy lexer, parser
and AST now live in `src/legacy/syntax.rs`. The entry point first gates the
explicit [typed-preview compiler and runner](../../spec/typed-preview.md) in `src/frontend/`.
Default execution still uses the legacy interpreter, module loader and CLI. Runtime services are split into `artifact`,
`data`, `network`, `packages`, and `benchmark` modules.

Legacy source execution parses into the dynamic `Value` interpreter. Numbers are `f64`;
arrays and records have shared `Rc<RefCell<...>>` storage. There is no OIR/CFG,
ownership checker, or `.ox` native machine-code backend in this path. The independent opt-in bool/i32/unit pipeline now has typed HIR, verified cyclic
Branch/Goto OIR, explicit bool value merges, initialized typed scalar places and calls/returns. Check remains non-executing; explicit
typed run uses bounded iterative scalar reference execution. Source-facing ownership
analysis remains unavailable; a private sealed ownership verifier and its reference
and LLVM consumers are implemented and qualified through raw OIR fixtures. See
[owned-consumer validation](owned-consumers-validation.md) for their evidence and limits.
A narrower explicit [LLVM native preview](../../spec/native-preview.md) compiles bounded, nonrecursive scalar programs, including checked i32 `+`, `-`, `*`, explicit i32/bool comparisons short-circuit boolean logic, mutable scalar locals and guarded while loops, to Linux x86_64 ELF PIE. See
[RFC 0004](../../rfcs/0004-bounded-reference-execution.md) for exact execution limits
and [RFC 0005](../../rfcs/0005-exact-i32-literals.md) for exact i32 literals.
[RFC 0006](../../rfcs/0006-checked-i32-arithmetic.md) adds ordinary checked i32
addition, subtraction and multiplication, with profile-independent runtime overflow.
[RFC 0011](../../rfcs/0011-mutable-scalar-locals.md) adds initialized typed scalar
places and ordered statement assignment through both reference and native execution.
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
performance corpora, broader native compilation, provider dispatch, true self-hosting,
and native AI training remain open work. The future static-core design must use
its own specification rather than treating the legacy interpreter as its oracle.

Ordinary bool-condition while is an experimental end-to-end extension with cyclic
OIR dominance and shared one-million-operation reference/native fuel. Guarded
native modules retain static call-depth/storage bounds and add explicit 16 MiB
human-diagnostic/64 MiB LLVM-text ceilings. Existing acyclic native admission is
preserved; no native recursion or final-performance claim follows.
See [RFC 0012](../../rfcs/0012-while-runtime-fuel.md) and
[while validation](while-validation.md).

Unlabeled break/continue extend the while subset end to end. Four-outcome typed
flow summaries separate return from loop transfers; lexical targets lower to
existing charged Goto edges without unreachable joins. No ownership or new type
is implied. See [RFC 0013](../../rfcs/0013-loop-control.md) and
[loop-control evidence](loop-control-validation.md).

The private owned-type declaration facade supplies nominal record/field IDs and
checked fixed scalar layouts to a private raw ownership/loan verifier. That verifier
checks whole-owner availability, explicit argument preparation and exact loan/call
regions, and constructs a sealed immutable witness. Private bounded reference
execution and LLVM lowering consume that same witness through a sealed immutable
execution plan. Raw OIR fixtures qualify aggregate storage, argument snapshots,
ownership transfers, call-bounded loans and checked resource/fuel accounting.
Struct/borrow source syntax and source-facing ownership checking and execution
remain unavailable. Existing scalar and legacy behavior remains unchanged, and
the public feature inventory is unchanged. See
[RFC 0014](../../rfcs/0014-owned-structs-call-borrows.md),
[groundwork validation](owned-types-validation.md),
[raw verifier validation](owned-verifier-validation.md), and
[owned-consumer validation](owned-consumers-validation.md).
