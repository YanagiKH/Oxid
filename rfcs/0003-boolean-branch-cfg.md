# RFC 0003: boolean branches and acyclic joins in typed-preview

Status: restricted experimental implementation proposal for review. These rules
apply only to the opt-in check-only preview, not a durable language edition,
completed M1/M2/M3 milestone or accepted ownership/execution policy. This follows
[RFC 0002](0002-verified-straight-line-oir.md); the full roadmap is unchanged.

## Source contract

Add statement-form `if expression { statements }` with an optional
`else { statements }`. The condition must be bool. No truthiness, else-if,
if-expression, trailing semicolon, naked block, loop, mutation or numeric rule
is introduced. Both arms are checked even for literal conditions. Missing else
means a reachable empty false path; empty arms are legal.

Parameters occupy the outer function scope. The function body and each arm have
lexical scopes. Initializers resolve before their new binding. Names from active
ancestors or top-level functions cannot be shadowed. Sibling arms may separately
bind the same name, and an outer declaration may reuse a closed child's name.
Each declaration still receives a distinct deterministic body-local ID. Child
and sibling bindings cannot escape their scopes.

A block returns if it reaches a return or an if whose two arms both return.
Statements following such a statement produce E0303. An if without else always
falls through. Every function, including unit functions, must explicitly return
on every reachable intraprocedural path; otherwise E0302 points at the function's
closing brace. Calls are assumed to return normally for this analysis, so direct
and mutual recursion remain legal without a termination claim. Flat-source
E0302/E0303 wording and all existing public diagnostic codes/ranges are retained;
E0300 also covers a non-bool condition at its full expression span. Invalid
placements of the newly recognized if/else keywords now receive syntax E0100
rather than their former unsupported-keyword E0101.

## Representation and lowering

AST and HIR own block arenas with ID edges, avoiding recursive block ownership.
HIR local/expression IDs follow lexical depth-first order with expressions in
child-before-parent order. Resolution and later statement traversal use explicit
frames. One expression cursor advances through condition, then arm, else arm and
later statements exactly once. Successful private typed tables also record each
block's return flow; immutable view alignment is checked in release builds.

New OIR terminators are Branch(condition, then, else) and Goto(target). Existing
Call and Return are unchanged. Condition calls precede Branch. Arm expressions
occur only on their own path. Lowering reserves arm entries and creates a join
only if some path continues. A falling-through arm ends in Goto(join); a returning
arm has no Goto. With no source else, Branch's false target is the join itself.
Two returning arms create no join. Later statements lower only into the reachable
join. Table order need not be topological; reserved joins can precede arm call
continuations. No phi, block argument, implicit return, execution or optimization
is introduced.

Existing origins remain unchanged. Branch uses the full if statement span and
its operand uses the condition expression span. Arm-entry blocks use their full
source block; arm-end Gotos use that block's closing brace. Synthetic join blocks
use the full if statement. Calls within arms retain call-origin continuations.
Exact UTF-8/CRLF provenance is tested separately from span validity.

## Verifier contract

Validate aggregate limits, identities, spans, references, types, signatures and
terminators in every raw block, including unreachable blocks. Branch requires a
bool condition and two valid successors; Goto requires one. Equal Branch targets
are legal. Reachable intraprocedural cycles and unreachable blocks are invalid;
recursive calls are not CFG edges. Every finite acyclic path therefore reaches
an explicit Return because all other terminators have successors.

Each non-parameter slot has at most one definition globally. Opposite-arm writes
to the same slot still fail AlreadyInitialized, as do parameter overwrites.
Unused undefined slots are legal, but reads require initialization. A definition
must dominate its use; same-block assignments must precede that use, with RHS
reads preceding destination initialization. Call results require strict dominance
by the call block: its one normal successor edge must have been crossed. A result
from one branch cannot be read at a join that has a bypass path. Source lexical
scope and IR dominance are separate invariants.

Use a deterministic topological traversal and predecessor-LCA dominator tree with
binary lifting, followed by iterative ancestor intervals. No blocks-by-locals
matrix, cloned initialization set or quadratic parent walking is permitted.
Definitions are scanned once before checking reads. The exact deterministic
first-error ordering is internal; public source diagnostics retain their earlier
phases and interface. Invalid raw origins are filtered before diagnostic rendering.

## Bounds and evidence

The function body counts as statement frame 1. At most 64 active source blocks
are allowed, independently of the existing 64 expression parser frames. Nested
syntax errors synchronize to later top-level functions under the existing
100-diagnostic cap. Arena ownership and iterative later traversals avoid
recursive-drop chains and unbounded CFG stack growth.

For F functions, C calls, I if statements, P parameters, L lets, X expressions and
Rb bare returns: locals = P + L + X + Rb; assignments = X - C + L + Rb;
blocks <= F + C + 3I. Thus locals/assignments retain their aggregate 100,000 cap,
and blocks have a 300,000 cap derived from the existing parser node limit.
Edges are at most twice the block count. Checked graph/scratch dimensions guard
all arithmetic before allocation. Binary lifting needs at most 19 levels and
5,700,000 usize cells (45.6 MB on 64-bit) at the block cap. Overall scratch is
O(locals + blocks log blocks + edges) per function and time is
O(functions + locals + assignments + operands + (blocks + edges) log blocks).
Peak current scratch is B*levels + 7B + E + 1 usize cells plus L optional
definition records: about 69.6 MB at the maxima on this 64-bit host, excluding
raw IR/source storage, small vector headers and allocator overhead.
Host allocation failure, blocking reads and user-program termination are outside
these engineering bounds.

Acceptance requires source-to-production checking, exact graph/provenance tests,
negative scope/type/return/shape cases, an independent small-DAG dominance oracle,
permuted IDs, call-edge tests, long/wide combined resource cases and retained
legacy/edition/JSON regressions. New fixtures stay embedded in Rust tests so
legacy recursive .ox discovery is unchanged. Local Linux evidence and later
publication CI are separate claims. See the increment's local validation report.

Stop at verified check-only boolean CFGs. Numeric literals, reference execution,
ownership, ABI/backend selection, native compilation, self-hosting and AI remain
separate roadmap work.
