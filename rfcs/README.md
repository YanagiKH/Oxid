# Oxid RFCs

Use an RFC to propose a language or toolchain contract before changing its
semantics. Record the acceptance decision explicitly; an implementation alone
does not imply approval.

A behavior-changing RFC should record:

1. Motivation, bounded scope, owner, and reviewer
2. Grammar/API and exact semantic rules, including errors and resource limits
3. Legacy compatibility, edition/artifact/ABI version impact, and migration
4. Alternatives, safety/trust boundaries, and performance/memory costs
5. Positive, negative, boundary, and regression acceptance cases
6. Supported host/target matrix and required hardware evidence
7. Implementation entry points, evidence, open questions, and acceptance decision

The initial implementation direction is a static, ownership-based core with
HIR and an analyzable OIR/CFG, followed by a first LLVM native backend. Existing
dynamic behavior remains a separately characterized legacy contract. This
states a design direction, not an accepted complete memory model or implemented
backend.

Small independently reviewable changes should link their affected specifications
and tests. Claims of production readiness or safety need evidence for the declared scope
and targets, beyond an accepted design or a passing unit test.

[RFC 0014](0014-owned-structs-call-borrows.md) records the experimental owned
record and call-only borrowing contract. [RFC 0015](0015-bounded-typed-projects.md)
extends it to bounded typed projects. Both have production source paths;
their validation ledgers distinguish actual activation from earlier groundwork.

[RFC 0016](0016-fixed-scalar-arrays.md) records the experimental fixed scalar
array contract, now exposed through explicit typed-preview check/run/native
compile. The [public-route ledger](../docs/architecture/fixed-array-public-validation.md)
distinguishes its source qualification from earlier private identity/layout,
raw-verifier and gated-consumer evidence.

[RFC 0019](0019-borrowed-scalar-slices.md) extends that path with call-only
`&[T]` / `&mut [T]` views of complete fixed scalar arrays, checked indexing,
runtime length and explicit reborrows. Owned unsized values, ranges, subslices,
element borrows and escaping references remain excluded. Existing source,
resource and native host gates remain in force. Its acceptance controls and
[three-module sample](../fixtures/typed-slice-samples/README.md) do not establish
green exact-head hosted CI or a completed milestone; historical ledgers retain
their original source and compiler qualification identities.

[RFC 0020](0020-owned-record-composition.md) adds bounded by-value record
composition, complete owned constructor initializers and named-root scalar
leaf/contained-array access. Ownership and call borrowing remain whole-root;
aggregate extraction/replacement and projected borrowing remain excluded.

[RFC 0021](0021-checked-i32-unary-negation.md) extends checked i32 arithmetic
with general prefix negation while preserving signed decimal literal behavior.

[RFC 0022](0022-projected-array-slices.md) connects record-contained fixed scalar
arrays to call-only slice helpers, retaining conservative whole-root authority.

[RFC 0023](0023-bounded-enum-match.md) adds bounded nominal move-only enums
with scalar payloads and exhaustive consuming statement matches. The public
typed-preview check/run/native-compile and formatter paths are experimental;
current-source qualification and exact-head hosted CI remain separate pending
gates. The [two-file scanner](../tests/fixtures/bounded_enum_scanner/main.ox)
returns 115 from fixed integer-code input, without a compiler-provider or
self-hosting claim.

[RFC 0026](0026-owned-native-storage-plan.md) makes the existing owned LLVM
allocation layout an explicit checked, witness-bound description. Phase 1
preserves admission, representation, ABI and fuel; native-specific admission
remains a separate proposal.

[RFC 0029](0029-package-sha256-integrity.md) proposes SHA-256 package-tree
integrity with explicit legacy-FNV migration; independent acceptance is pending.

[RFC 0030](0030-bounded-u8-scalar-core.md) approves a bounded unsigned-byte scalar
contract for implementation planning: explicit checked narrowing, exact widening,
unsigned comparison and scalar helper transport. Byte aggregates/arithmetic and
I/O migration remain excluded. Implementation and work/byte resource admission
remain unqualified; the contract records explicit type-name migration and
unchanged producer-protocol boundaries.

[RFC 0032](0032-typed-project-workflow.md) defines manifest-selected typed
project commands and managed native output. [RFC 0033](0033-typed-path-dependencies.md)
extends that workflow to local dependency packages in one checking session,
with package-local roots, nominal identities and ordinary visibility rules.

[RFC 0034](0034-typed-local-locking.md) adds a separate typed local lock format,
locked/offline/frozen project operations and checksums of the actual retained
dependency sources used for execution.

[RFC 0035](0035-typed-workspaces.md) adds bounded workspace membership and
selection, real library checks and a shared input lock across the complete
canonical package union.

[RFC 0036](0036-native-object-reuse.md) adds opt-in reuse of native LLVM objects,
with toolchain/environment identities, immutable cache blobs and fresh runtime
compilation and linking on every build.

[RFC 0037](0037-native-doctor.md) adds native toolchain diagnosis through real
SDK compilation, fixed-program compilation/link/execution and typed package
input checks, with bounded temporary probes and versioned diagnostic output.

[RFC 0038](0038-typed-source-packages.md) adds deterministic retained source
bundles, strict bounded verification, complete local layout relocation and
atomic creation/extraction with existing typed locks.

[RFC 0039](0039-reproducible-native-builds.md) adds resolved native build settings
and opt-in project-relative runtime diagnostics for reproducible bounded native
builds across preserved local source layouts.

[RFC 0040](0040-native-build-profiles.md) defines development and optimized
release native builds with the same language checks and execution limits.

[RFC 0041](0041-owned-enum-payloads.md) extends consuming enum matches to
finite record, fixed-array and nested enum payloads, with whole-value ownership
and bounded containment analysis.
