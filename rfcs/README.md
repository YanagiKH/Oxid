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
