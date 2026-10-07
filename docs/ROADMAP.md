# Roadmap

Oxid's long-term direction is a statically checked, native-compiled language with its own toolchain. The current 0.9 release is an experimental interpreter. This page separates the working baseline from the development ahead; it is not a release-date commitment.

## Available in 0.9

- Source interpretation and deterministic OXBC 1.0 serialized-AST artifacts
- Functions, loops, pipelines, records, JSON, and lazy task values
- Files, processes, TCP/HTTP helpers, and cross-language process adapters
- Local and commit-pinned Git dependencies with version 1 lockfiles
- Project scaffolding, checks, tests, formatting, and internal benchmarks

See the [implementation baseline](architecture/current-baseline.md) for limitations. The Oxid textual emitter and legacy bootstrap commands are demonstrations and serialization checks; they do not establish compiler self-hosting.

## Development sequence

| Stage | Work | Required evidence |
|---|---|---|
| M0: establish the baseline | Inventory features, characterize legacy behavior, write specifications and trust boundaries | Reproducible tests, scoped claims, reviewed contracts |
| Frontend and static core | Modular frontend, typed AST/HIR, analyzable IR, ownership and memory rules | Positive and compile-fail tests for the defined safe subset |
| Native compilation | First LLVM backend, native CLI, files, processes, and FFI contracts | Executed native programs on declared targets |
| Runtime and ecosystem | Concurrent scheduling, Web services, packages, debugging, and migration tools | Representative applications and failure/recovery tests |
| Compatibility and self-hosting | Scoped Rust migration, production provider dispatch, compiler rebuilds | Compatibility corpus, no hidden fallback, compiler fixed point, rebuild without Rust |
| Domain validation | Production pilots and broader platform support | Domain-specific correctness, security, deployment, and performance results |

M0 is in progress. The current inventory and selected legacy tests cover only part of that stage. New semantics require [RFCs](../rfcs/README.md); the legacy interpreter is not the specification for the future static core.

Typed-project public dispatch now uses the shared declaration/type schedule and
existing verified reference/native consumers. The
[Unit4 ledger](architecture/typed-project-unit4-validation.md) separates exact
current-source qualification from archived private-core evidence and future host
work. This bounded capability advances the static frontend; it does not complete
a roadmap milestone or v1.0.

The [bounded expression component](../fixtures/typed-expression-samples/README.md)
exercises the current language with an iterative parser, a 15-node expression
arena, and a separate evaluator. Its public reference/native example builds
seven nodes and returns 39 for `12 + 3 * (4 + 5)`. This is a compiler-component
exercise over fixed input, not a compiler provider, rebuild or self-hosting claim.

The companion [stack-code component](../fixtures/typed-expression-samples/STACK_CODE.md)
lowers that arena to at most 15 instructions and validates/executes the instruction
buffer independently of tree links. Its stdin entry uses the existing bounded
input operation. Exact rows and malformed instruction fixtures establish this
small producer/consumer boundary; they do not establish a production backend or
self-hosting.

The [bounded stdout/process contract](../rfcs/0025-bounded-stdout-process-entry.md)
turns the component into a persistent producer/consumer boundary: one executable
writes an exact 80-byte artifact, and separate Oxid and external readers validate
and execute only saved bytes. This adds reusable process output and explicit exit
statuses without extending the sample ISA. The next acceptance gate is current
source and exact-head hosted qualification, not another instruction-set demo.
General file management, a production compiler provider and self-hosting remain
separate roadmap work.

The [bounded typed-preview lexer](../fixtures/typed-lexer-samples/README.md) is an
Oxid-written frontend component for at most 128 ASCII source bytes. Its token
columns preserve trivia and byte spans, and its transcript distinguishes complete
lexical diagnostics from transport failures. Canonical Rust comparison and
reference/native execution are separate checks. It does not switch production
providers or extend the contract to Unicode; the next consumer can use this token
interface for a real typed-preview parser.

The [bounded scalar typed-preview parser](../fixtures/typed-parser-samples/README.md)
now consumes that token interface in Oxid. It implements scalar functions,
expressions, calls, bindings and control flow for at most 128 ASCII source bytes,
with exact AST/spans and first diagnostics compared against the unchanged Rust
parser. Its fixed reference/native roster has 139 inputs and zero pending grammar
families. Native admission uses the separately qualified inventory contract;
physical storage and language fuel are unchanged. Hosted qualification, provider
selection, source provenance and broader grammar remain distinct gates. This is
a reusable frontend component, not a compiler self-rebuild claim.

The separate [bounded scalar static frontend](../fixtures/typed-static-samples/README.md)
adds complete AST1 validation, name resolution, type checking and flow observation
for the same 128-byte ASCII scalar grammar. The existing parser and a separate
Oxid consumer exchange exact source/OPA1 bytes; the host only frames and relays
them. Local reference/native checks cover 77 semantic pairs (29 complete typed
fact results and 48 first diagnostics covering all 14 kinds), with four producer
failures/refusals recorded separately. Independent validator and I/O controls
cover the untrusted source/AST boundary. The consumer admits at I = 7,741 and
W = 3,907 under unchanged limits; the combined source-to-type candidate remains
refused at I = 8,778.

This is concrete progress toward a reusable frontend: syntax and complete bounded
static observations now have explicit component boundaries. Integrated
source/projection review has passed for the bounded scope, including independent
re-projection of all 77 retained semantic pairs. The reproducible current-source
controller passes 81 cases in each mode, 81 wire pairs and 48 I/O pairs; its
independent review is clear. Exact-head hosted CI remains pending. Production
provider selection, authoritative source ownership,
`TypedProgram`/OIR construction and compiler self-rebuilds still require their own
gates; this component does not complete a roadmap milestone.

## Native AI work

Native AI support needs typed buffers and CPU tensors first, followed by automatic differentiation, optimizers, and complete training loops. GPU execution, tensor compilation, multi-device training, and resumable checkpoints add separate hardware and correctness requirements. Calling an external framework through a process adapter does not satisfy these milestones.

## How progress is reported

The [feature inventory](feature-status.json) links implementations, tests, evidence, and gaps. A feature becomes validated only for its declared scope and tested targets. Native compilation, self-hosting, Rust compatibility, production readiness, and native AI training each need their own evidence.

Performance claims require equivalent workloads and recorded environments. The current [benchmark harness](BENCHMARKS.md) measures Oxid itself; it does not establish a general speed advantage over Rust.
