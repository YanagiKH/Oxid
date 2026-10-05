# RFC 0021: checked i32 unary negation

Status: bounded experimental increment under delegated language development;
implementation and independent review pending. This extends RFCs 0005, 0006,
0008 and 0020 without claiming a stable language, completed milestone or v1.0.
Baseline is PR34 head c4c4ec0958a31fc7878bdc4031e93b204ff14acb,
tree b53f5e4969ed6036db79c1c0456f6d541301b0b0. Baseline CI was running, not
confirmed merged, when this work began.

## Contract

General prefix `-` accepts only i32 and returns i32. It binds at the same tier
as boolean `!`, above product operators. Prefixes associate right to left.
However, a minus whose next nontrivia token is a decimal candidate retains the
existing signed-literal primary rule. Its spelling/range validation, complete
origin and fuel costs are unchanged. This includes comments/whitespace, leading
zeros, negative zero and MIN. No positive unrepresentable intermediate is formed.

Examples: `-x`, `-(x + 1)`, `-f()`, `--1`, `1---2` and `!-x == true` parse
(the last is a type error, since ! requires bool). `--2147483648` negates the
valid MIN literal and fails at runtime; `-(2147483648)` fails E0203 resolution
before execution, including in dead code. Unary plus, casts, other integer
widths, floats, bitwise operations and compound assignment remain excluded.

The operand is fully evaluated exactly once before the operation. First-error
and short-circuit ordering are unchanged. The mathematical negation must fit
i32: MIN fails with existing E0604/oir-run and message `checked i32 arithmetic
overflow`, at the one-byte prefix minus. A bool, unit or owned value fails E0300
at its operand span. The full unary expression span includes all trivia and
operand grouping. Existing reference-parameter value restrictions still apply.

## Representation and resources

AST, scalar HIR and owned HIR have a unary node with one child and a real
operator origin. The existing expression-height bound 64 includes this node;
prefix scanning remains iterative with the existing pre-entry nesting bound.
Signed decimal literals do not gain any node. Formatter role marking treats
unary minus as tight-after while preserving exact tokens and comment anchors.
Module/source ownership validation walks the operand and operator spans.

General negation lowers to one shared CheckedNegateI32 assignment with one
operand. It uses the ordinary assignment fuel charge before reads/overflow and
one ordinary temporary local. No extra constant/local or synthetic source origin
is introduced. The independent verifier checks destination/operand i32 types,
valid origins, initialized uses and dominance. The immutable verified witness
remains mandatory on scalar and owned routes. Native consumers use LLVM checked
subtraction with immediate zero, preserving checked overflow and one charge;
there is no source interpretation fallback or unchecked LLVM negation.

Existing runtime caps and preflight formulas remain. Private metadata layout
measurements and exact fuel boundaries belong in the validation evidence, not
assumed from source payload sizes. The prefix stack remains a Vec<Span>, as
before, rather than adding a larger token record.

## Compatibility and qualification

Legacy edition, dependencies, OXBC, ABI/CLI options and scalar result output are
unchanged. Current tests that rejected general unary syntax require narrow
successor expectations. Historical binding/oracle artifacts remain immutable;
named changed observations must be reported for a separate narrow successor.
Qualification covers signed-literal regression, precedence, types, error spans,
MIN, operand order/single evaluation, short circuit, exact fuel and depth limits,
formatter/module parity, malformed raw IR and source-free native execution.
Local Linux Rust 1.99 / LLVM 19 evidence does not claim hosted or other-host CI.

Rejected alternatives: lowering to a synthetic `0 - operand` expression adds
costs or unauthenticated origins; duplicating the operand in a binary node risks
repeated evaluation; folding signs changes overflow/fuel; extending positive
literal range through grouping violates the existing literal validity contract.
