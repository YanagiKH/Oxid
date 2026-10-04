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

[RFC 0016](0016-fixed-scalar-arrays.md) proposes fixed scalar arrays and checked
indexing. Its preparatory units implement private identity/layout, raw verification
and gated reference/native consumers. Public array syntax and production raw
array admission remain disabled; the validation ledgers distinguish local
consumer execution and combined controller admission from hosted qualification.
