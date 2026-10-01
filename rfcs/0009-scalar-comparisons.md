# RFC 0009: exact typed scalar comparisons

Status: accepted scope and operator/type policy, 2026-10-01; experimental
implementation for independent review. Extends [RFC 0006](0006-checked-i32-arithmetic.md)
and [RFC 0008](0008-native-checked-i32.md). Owner/reviewer: unassigned.
No complete numeric system, generic equality, Rust compatibility, ownership,
production safety, stable ABI or roadmap milestone is certified by this slice.

## Accepted behavior

`==` and `!=` accept i32/i32 or bool/bool. `<`, `<=`, `>` and `>=` accept only
i32/i32 and use signed mathematical order. Every comparison returns bool.
There is no implicit conversion, truthiness, promotion, float intermediate,
subtract-to-compare implementation or unit equality. Mixed i32/bool, either unit
operand, and ordered bool comparisons fail E0300 even in unused declarations
and unchosen branches.

The type checker inspects operand types in source order. For equality, an
unsupported left type is rejected first; otherwise the right type must match the
supported left type. Ordering requires i32 for each operand. Type errors point
to the first invalid operand expression. Existing errors in operand subexpressions
retain their ordinary earlier diagnostic behavior.

All six operators share one non-associative precedence tier below `+`, `-` and
`*`. Parentheses start a complete nested expression and can explicitly combine
comparison results using bool equality.

```text
expression := sum (comparison sum)?
comparison := "==" | "!=" | "<" | "<=" | ">" | ">="
sum        := product (("+" | "-") product)*
product    := primary ("*" primary)*
primary    := "true" | "false" | name | "(" ")" | "(" expression ")"
            | name "(" arguments? ")" | decimal | "-" decimal
```

`1 + 2 < 4 * 2` compares the arithmetic results. `(1 < 2) == true` and
`true != (3 >= 4)` are legal bool comparisons. `1 < 2 < 3`, `1 == 2 < 3` and
`1 < 2 == true` fail E0100/parse at the second comparator, with its complete
one- or two-byte span. No chain is silently reassociated or converted to an
implicit conjunction. `(1 < 2) < 3` instead parses and fails E0300 at the bool
left operand. Multicharacter operators require adjacent bytes: trivia cannot
join `! =` or `< =`. Standalone `!`, `&&`, `||`, `and` and `or` remain unsupported.
Assignment `=`, function arrow `->`, literal-only minus and all earlier arithmetic
precedence/associativity rules are unchanged.

Both operand subtrees execute exactly once, completely left before right. Then
the comparison executes. Comparisons do not short-circuit, even when the left
value or eventual discarded result makes the answer predictable. The first
operand error stops evaluation. `0 == (2147483647 + 1)` raises E0604 at `+`;
comparison itself cannot overflow or introduce a new runtime failure category.
Unchosen branches do not execute; discarded comparisons do execute their operands.

## Representation and independent verification

AST/HIR use a closed `ComparisonOp` and comparison expression with ordered child
IDs, whole-expression origin and separate operator origin. Each comparison is one
expression node and participates in the existing total tree-height limit of 64.
The resolver completes the left subtree first; lowering preserves calls and
continuations without duplicating operands.

Private OIR adds `CompareScalar`, an ordinary assignment with two typed operands,
a comparison operator and its origin. The independent verifier checks both
operand IDs/spans, the explicit same-type i32/bool equality or i32-only ordering
table, bool destination, valid operator span, global definition uniqueness and
both ordered dominance/read-before-write checks. Unit equality is rejected by
this independent consumer even if a faulty producer emits it. As before, span
validity does not prove arbitrary raw IR matches source spelling.

Reference execution reads concrete scalar slots, matches i32/i32 for all six
operators and bool/bool only for equality/inequality, and directly compares those
values. It does not use blanket host `Scalar` equality that would accidentally
include unit. Impossible operand/operator pairs are E0500 invariant failures;
there is no raw public OIR execution route.

One comparison assignment costs one fuel, charged before operand reads; its
operand expressions, calls and allocations have their existing separate costs.
`return 1 < 2;` and `return true == false;` each need eight fuel including root
allocation and Return. `return (1 < 2) == true;` needs fourteen, including the
existing explicit grouping copy. No slot representation, scratch allocation,
frame/slot ceiling or reference budget changes.

## Native lowering and compatibility

The bounded native allowlist explicitly admits `CompareScalar`. Equality uses
LLVM `icmp eq`/`ne` on the verified operand width, i32 or i1. Ordering uses
`icmp slt`/`sle`/`sgt`/`sge` on i32. All results are i1. The predicates and result
representation follow the [LLVM 19 icmp contract](https://releases.llvm.org/19.1.0/docs/LangRef.html#icmp-instruction).
No subtraction, unsigned ordering, floating comparison, overflow flag or new
C adapter implements the semantics.

This scalar assignment requires no Phi/value join, cyclic CFG, new runtime
symbol, native exception path or ABI revision. Existing bool output returns
status 0 for both true and false. Operand arithmetic failures retain exact
reference/native E0604 human output, operator origin and exit 1; native diagnostic
I/O failure remains 74. Native O0/tool trust, whole-file nonrecursive admission,
all static bounds, explicit output routing and atomic no-clobber publication are
unchanged. The native cost recurrence already counts comparison once.

Legacy dynamic equality/order and OXBC bytes remain independent and unchanged.
Typed sources previously rejected for these operators can now pass within this
explicit table. Existing tests that asserted comparisons were unsupported are
replaced with still-unsupported logical-operator controls; the legacy tests stay.
Other scalar types, generalized equality, boolean negation/short-circuiting,
loops/mutation and native recursion require their own decisions and evidence.

## Acceptance evidence

[The comparison validation report](../docs/architecture/scalar-comparison-validation.md)
records full scalar/operator tables, signed MIN/MAX boundaries, parser chains and
limits, raw-IR adversaries, exact fuel/order, Python tagged-value expectations,
reference/native profile parity and standalone output. Python uses explicit type
tags and exact host types so its bool-as-int inheritance cannot validate coercions.
Local tests, independent review and hosted CI are separate gates.
