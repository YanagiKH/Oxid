# RFC 0010: bool logic and short-circuit value joins

Status: accepted for restricted experimental implementation on 2026-10-01.
The user approved bool-only `!`, `&&`, `||`, conventional precedence and runtime
short-circuiting with full RHS type checking. This extends [RFC 0009](0009-scalar-comparisons.md)
and the acyclic CFG of [RFC 0003](0003-boolean-branch-cfg.md); it is not a stable
edition, completed roadmap milestone, ownership policy or production certificate.

## Source semantics

`!` consumes bool and returns its inverse. `&&` and `||` consume bool/bool and
return bool. There is no i32/unit truthiness or coercion. `a && b` evaluates a
once, returning false if a is false and otherwise evaluating/returning b.
`a || b` evaluates a once, returning true if a is true and otherwise
evaluating/returning b. Executed work retains source order and first-error rules.
Discarded results still execute their selected path. Skipped RHS arithmetic,
functions and call arguments do not execute or produce runtime errors.

Precedence, tightest first: primary/call/grouping, `!`, `*`, `+`/`-`, comparisons,
`&&`, `||`. Binary logical levels associate left; prefix `!` nests right. All
comparisons remain at one non-associative tier. Thus `a || b && c` groups as
`a || (b && c)`, `!a == b` as `(!a) == b`, and `!(1 < 2)` is valid while `!1 < 2`
is a bool operand error. Arithmetic/general-unary-minus semantics are unchanged.

The lexer recognizes adjacent `&&`, `||`, `!` and longest-match `!=`. Separated
`& &`, `| |`, bitwise operators and textual `and`/`or` remain unsupported.
Previously unsupported `!` in invalid grammar positions now produces ordinary
E0100 syntax diagnostics rather than E0101. All operands, unused declarations
and unchosen branches are still resolved/type checked, including a literal-
short-circuited RHS; existing phase ordering takes precedence over runtime order.

No if-expression, loop, mutation, generic operator/type machinery, native
recursion, alternate numeric behavior or optimization mode is introduced.

## HIR and root-directed lowering

AST/HIR retain a closed LogicalOp (And/Or), Not and Logical nodes with ordered
children and exact operator origins. Prefix parsing is bounded before recursive
entry; total height still includes every operator/group/call and is capped at 64.
HIR remains lexical child-before-parent order and the type checker visits all
expressions. Lowering uses explicit expression continuation frames from each
statement root, rather than eagerly draining the flat HIR tape. A monotone
postorder completion cursor still checks each HIR node is emitted once.

Unary negation becomes one NotBool assignment. Each logical expression reserves
an RHS entry and a join. After lowering left, its final block branches to RHS or
join according to the operator. The actual final RHS block, after any nested
logic/calls, ends with Goto(join). The join defines the result from left on the
short-circuit edge and right on the evaluated edge. No constant/continuation
copying, opposite-arm destination writes or eager logical Rvalue is used.

## Explicit independently verified bool merge

A BasicBlock has at most one optional BoolMerge, containing a destination, two
(predecessor, operand) inputs, a full expression origin and its operator origin.
This fixed two-input bool shape is intentionally narrower than a generic phi.
It uses the existing expression temporary; no extra local slot is allocated.

The verifier independently validates every destination/input type and ID, all
origins, and both predecessor IDs before CFG analysis, even in unreachable raw
blocks. A merge is forbidden at function entry and requires exactly two distinct
incoming CFG edges, matching the two distinct listed predecessors. Equal-target
Branch remains legal for a non-merge target. Repeated edges into a merge are
rejected; source logical lowering does not produce them.

A merge defines its destination once at block entry. It participates in global
single-definition checks with parameters, assignments and call results. Normal
reads can use it in its block or dominated blocks. Each incoming operand must
instead be available on its designated predecessor edge: parameters and dominating
assignments/merges qualify, including assignments earlier in the predecessor.
A call result defined in that predecessor qualifies only on its normal successor
edge to this join, after the call returns; it remains unavailable in its own
arguments/statements. Earlier call results require strict dominance of the
predecessor by their call block. A value from the opposite arm cannot qualify.

The existing iterative reachability/topological/dominator proof rejects cycles
and unreachable blocks. Fixed merge pairs add linear work/storage without a
blocks-by-locals matrix, cloned initialized sets or production path enumeration.
Independent tests use path removal over small DAGs, including reversed block IDs,
to check assignment and normal-call-edge availability without using dominators.

## Execution, fuel and bounds

An activation records its actual predecessor and whether block-entry merging is
pending. Branch/Goto record the departed block; a normal call return records the
call block. On merge entry, charge one fuel before selecting/reading only the
matching input and writing its destination once. Then ordinary assignments run.
No unselected incoming operand is read. Unary NotBool also costs one assignment.

Ungrouped `return !true;` requires 6 fuel; `return false && true;` and
`return true || false;` require 8; `return true && false;` and
`return false || true;` require 10. Parentheses retain their existing temporary
and copy cost. Short-circuiting skips RHS instruction/call charges; activation
still preallocates/charges the whole function's bounded local table, including
RHS temporaries. Existing programs' fuel counts and failure precedence remain.

For F functions, C calls, I if statements, S logical expressions, P parameters,
L lets, X expressions and Rb bare returns:

- locals = P + L + X + Rb
- assignments, including merges = X - C + L + Rb
- blocks <= F + C + 3I + 2S
- edges <= 2 * blocks

Allocation preflight and raw verification count merges toward the existing
100,000-assignment cap. The existing 100,000-local and 300,000-block caps and all
source/parser/reference limits remain. The bounded dominator algorithm and its
asymptotic scratch/time bounds are unchanged; the fixed pair is linear overhead.

## Native lowering and admission

Only verified NotBool and the fixed bool merge enter native lowering. NotBool
uses `xor i1`; logical control flow uses Branch/Goto and `phi i1`, not eager
and/or/select. Phi is emitted before ordinary instructions in its LLVM block.
Checked arithmetic expands an OIR block into overflow/success LLVM blocks, so an
explicit deterministic map chooses its actual exit label: the final checked
success label if present, otherwise its original bN. Phi inputs use these labels,
including when later copies/comparisons/calls follow checked arithmetic.

The unchanged O0 LLVM verifier/tool pipeline validates the resulting module.
Native fuel sums every block's ordinary assignments, optional merge and terminator,
plus existing call/argument/allocation costs. Both direct logical paths have a
conservative bound 10, even when a literal left operand skips RHS at runtime.
All skipped RHS calls still participate in whole-file recursion/depth/slot/fuel
admission. Thus a source with a skipped recursive RHS may run under the reference
executor but is rejected before native tools, just like an unchosen recursive if.
All stricter native ceilings, trusted-tool boundary, runtime ABI, exact E0604
stderr/exit 1, output failure 74 and atomic no-clobber publication remain unchanged.

## Origins and evidence

NotBool stores the `!` span separately from its full assignment span. Logical
Branch, RHS-ending Goto, BoolMerge and join block use the full logical expression
span; RHS entry uses its expression span; merge inputs retain operand spans.
Operator spans remain exact through AST/HIR/OIR. Existing arithmetic/call spans
are preserved. Span validity is not itself proof of source-to-IR equivalence.

Acceptance covers public truth tables/grammar/types/evaluation contexts, exact
selected first errors, raw adversarial merge tests, an independent small-DAG edge
oracle, hand-counted fuel/caps, real LLVM modules with post-overflow predecessors,
independent tagged lazy Python/reference/native differential tests, standalone
artifacts and all predecessor gates. See [the validation report](../docs/architecture/boolean-logic-validation.md)
for actual runs and bounded claims; hosted CI and independent review are separate.
