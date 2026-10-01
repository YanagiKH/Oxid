# RFC 0006: checked i32 addition, subtraction and multiplication

Status: accepted scope and overflow policy, 2026-10-01; experimental implementation
for independent review. Extends [RFC 0005](0005-exact-i32-literals.md).
Owner/reviewer assignment and evidence remain in the feature inventory. Acceptance
of this bounded language decision does not certify a complete numeric system,
Rust compatibility, ownership, native compilation or completed M2/M3 milestone.

## Decision and grammar

Ordinary binary `+`, `-` and `*` take two i32 operands and produce an i32.
Every operation checks its mathematical result against [-2147483648, 2147483647].
An out-of-range result stops execution with E0604/oir-run, `checked i32 arithmetic
overflow`, at that operator's single-byte source span. The policy is identical in
all host build profiles; no host debug assertion controls language overflow.
No wrapping or saturating behavior is implicit. Explicit wrapping operations,
division, remainder, shifts, casts and integer comparisons remain deferred.

```text
expression := product (("+" | "-") product)*
product    := primary ("*" primary)*
primary    := "true" | "false" | name | "(" ")" | "(" expression ")"
            | name "(" arguments? ")" | decimal | "-" decimal
```

Multiplication binds more tightly than addition/subtraction. Each precedence
level associates left; parentheses override this. A subtree's left operand is
fully evaluated before its right operand, and the operation runs after both.
Calls occur exactly once in that order. Evaluation stops at the first error;
there is no speculative evaluation, folding or algebraic reassociation. Thus
`2147483647 + 1 - 1` overflows, and `0 * (2147483647 + 1)` also overflows.
Discarded expressions still execute; unchosen branches do not execute.

The sign remains literal-only. `1--2` means subtraction of the signed literal
-2; `--1`, `-x`, `-(1)`, `-f()` and unary `+1` remain unavailable. Existing
trivia between sign/digits and exact MIN conversion remain unchanged. E0203
literal validity is separate from E0604 arithmetic overflow. Checking `MAX + 1`
succeeds without running it; literals outside i32 fail checking even in dead code.

No bool/unit coercion or truthiness is added. E0300 points to the first wrongly
typed arithmetic operand. Full-file name/type checking includes unused functions
and unchosen branches. Results may be passed, bound, discarded or returned with
existing scalar rules. Main's result remains printed data, never an exit status.

## Representation, verification and resources

AST/HIR arithmetic nodes retain an operator enum, ordered child IDs, the whole
expression span and the exact operator span. HIR resolution emits the entire left
subtree before the right. Lowering preserves that order, including explicit call
terminators/continuations. OIR adds a CheckedI32 Rvalue, with ordered Operand
values and operator origin; it is an ordinary assignment with no hidden calls.
The immutable verified witness remains mandatory. The independent verifier
checks both operand IDs/spans/types, operator span validity, i32 destination,
globally unique definitions and read-before-write/dominance in both positions.
An origin-validity proof does not establish that arbitrary raw IR matches source.

Reference evaluation reads both i32 slots, applies Rust's explicit checked integer
operations and writes only on success. It never converts through f64, wraps,
uses the legacy runtime, or relies on profile-dependent panic behavior.
E0604 is an ordinary runtime error (exit 1), distinct from E0500 internal invariant
failure (exit 2). JSON retains schema 1 with one diagnostic and failed run-summary,
result null. Text failures emit no partial result to stdout.

A binary expression is one parser node, one temporary local and one assignment.
Its assignment costs one fuel, charged before reads/overflow checking; operand
expressions have their own costs. Fuel exhaustion uses the full unperformed
assignment span; arithmetic overflow uses the operator span. `return 1 + 2;`
with no other expressions needs exactly eight fuel: four for root/three slots,
three assignments and one return. Limits and preflight formulas are unchanged.
Two reads per operation are constant bounded work and do not allocate scratch.

Total expression-tree height, including binary, grouping and call nodes, is
bounded to 64. The parser retains its separate pre-entry recursion bound and
checks child heights before adding a node. Flat left-associative chains cannot
create an unbounded recursive resolver traversal: 64 literal terms with 63
operators pass, 65 terms fail E0400/parse. A flat chain is not silently reassociated.
Temporary parser height storage is one usize per expression; on a 64-bit host
at most 800,000 bytes, excluding vector capacity and other compiler storage.
AST/HIR/OIR remain private, unversioned in-memory representations.

## Compatibility, alternatives and evidence

The default legacy edition, its f64 semantics, OXBC bytes, README layout, Cargo
dependencies and public CLI options remain unchanged. Previously unsupported
binary operators now compile in typed-preview; old unary/spelling/range rules
remain. Native support must be added explicitly by its own backend contract;
this increment supplies only checking and reference execution.

Alternatives rejected for ordinary arithmetic are wrapping, saturating, and a
profile-dependent checked-debug/wrapping-release split. They hide errors or make
one source behave differently by optimization profile. Widening before arithmetic
would change the provisional i32 contract. Arbitrary precision is used only by
an independent Python test oracle, not the implementation.

[Validation](../docs/architecture/i32-arithmetic-validation.md) maps the public
CLI, malformed raw-IR, exact fuel, provenance, order and independent bigint
corpora to fresh local results. The Python oracle also compares debug/release
JSON and exits on the same generated sources. Existing literal, scalar, CFG,
legacy and repository checks remain required. Public claims are limited to the
actually tested host; CI configuration is not evidence of remote results.
