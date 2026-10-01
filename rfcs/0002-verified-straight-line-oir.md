# RFC 0002: verified straight-line OIR for typed-preview

Status: experimental implementation proposal for review with this increment.
No stable edition, completed roadmap milestone, memory model or execution
contract is accepted by implementation alone. Maintainer owner/reviewer
assignments remain unassigned in the feature inventory.

## Bounded decision

Require every successful `oxid check --edition typed-preview` to lower the
actual successful typed HIR into typed local-slot OIR and independently verify
it. The narrow representation contains Assign, Call with an explicit normal
continuation, and Return. Its intraprocedural CFG is a reachable acyclic chain;
the program call graph may be recursive. Empty source stays valid.

The exact invariants, source provenance, error handling, complexity and limits
are specified in [the preview contract](../spec/typed-preview.md#verified-straight-line-oir).
This increment adds no source grammar, command, dump, public raw-IR loader,
execution, serialization, dependency, ownership analysis or numeric semantics.

## Compatibility and diagnostics

The preview stays opt-in and check-only. Existing source errors and successful
text/JSON output remain compatible. The early edition gate, default legacy
route, unsupported-command rejections, no fallback and no-output/cache behavior
remain unchanged. OXBC, the runtime and legacy AST are not inputs to this IR.

A newly detected compiler invariant failure is E0500, stage `oir-lower` or
`oir-verify`, exit 2; it explicitly identifies an internal compiler error.
Lowering budget failures use E0400, stage `oir-lower`, exit 1. Both preserve JSON
schema 1 and a failed summary with null function count. Malformed spans are
filtered before rendering. Arbitrary host/producer panics are not caught.

## Why this increment precedes branches

Calls already require real block boundaries: nested argument calls become
separate continuations in left-to-right order. Establishing this producer/
verifier boundary needs no new grammar, nested scopes, branch return rules or
join algorithm. An unused Branch variant or hand-built IR demonstration would
not establish the actual production boundary.

The current canonical form initializes each non-parameter slot once and never
overwrites parameters. One initialization bitmap represents the dominating
prefix of definitions along the unique chain. General CFG joins, storage
liveness, mutation and SSA decisions are deliberately deferred.

A separate signature table was rejected to avoid duplicated unchecked types.
Parameter types instead live in the validated prefix of each function's locals.
Reparsing source, adapting dynamic legacy values, optimizing away discarded
calls and emitting LLVM directly were rejected for this slice.

## Trust and resource boundary

Raw IR is private to the OIR subtree; the driver receives only an immutable
verified witness. The verifier checks all raw tables/blocks with fallible
lookups, then walks continuations iteratively with O(locals + blocks) per-function
scratch memory. It returns a deterministic structured failure on malformed IR
in debug and release builds. Source-derived provenance is tested separately from
span well-formedness. This is not an untrusted public artifact format.

Locals, blocks and assignments each have a checked aggregate 100,000 limit,
preflighted before lowering allocates storage/maps. The expansion is bounded by
distinct already-counted parser nodes, preserving the accepted source subset.
Call arguments remain bounded at 256. Limits do not guarantee host allocations
or establish an OS sandbox.

The verified property is structural intraprocedural return completeness assuming
every call returns normally. It says nothing about recursive termination, user
stack safety, executable call behavior or memory safety. The compiler itself
never follows the call graph and does not execute the program.

## Acceptance and evidence

Required checks cover real parsed/resolved/typed-source lowering; nested distinct
callees and argument order; discarded bool/unit calls; groups, local copies,
empty files, unit values and recursion; exact UTF-8/CRLF use/declaration spans;
and deterministic repeated lowering. Raw malformed-IR tests cover IDs, tables,
references, types, initialization, terminators, continuations, spans and bounds.
Long chains guard iterative traversal and single-bitmap state; full legacy,
edition-gate and typed regression suites remain required.

[Local validation](../docs/architecture/oir-validation.md) identifies the snapshot,
commands and host. CI for a later publication commit must be checked separately;
a local Linux result does not establish macOS, Windows, ARM64, installation or
Docker evidence. No benchmark or safety certification follows from these tests.

Stop at this boundary. Boolean branches, lexical scopes, return analysis and
join dataflow require a subsequent proposal and review before ownership or native
code work. The full roadmap remains unchanged.
