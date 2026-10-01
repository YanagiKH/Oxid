# RFC 0012: ordinary while and shared native runtime fuel

Status: experimental implementation. This is a bounded typed-preview extension,
not stable language semantics, an ownership milestone, a termination theorem or
final native performance mode. It extends RFC 0011's scalar places and RFC 0010's
bool merges. The default legacy frontend and OXBC serialization are unchanged.

## Source contract

`while expression { statements }` is a statement without trailing semicolon.
The condition must be bool and is evaluated before every iteration. False skips
the body, including on its first evaluation. Nested loops, existing if statements,
mutable bool/i32/unit state, calls, checked arithmetic and lazy logic retain their
ordinary evaluation order. Conditions and bodies are always resolved and checked,
including literal-false bodies. There is no truthiness, assignment expression,
while-else, loop expression, for, break, continue or labels in this increment.

Body declarations have their existing lexical block scope. Each dynamic execution
of a declaration evaluates its initializer and starts a fresh value in reusable
activation storage. Outer places persist; earlier immutable snapshots and copied
arguments never become aliases. Duplicate active names and shadowing remain errors.
Parameters and ordinary lets remain immutable source bindings.

Return inside a loop exits its function. For static return completeness, every
while retains its false edge, even with literal true. A while does not by itself
establish a terminal return; the enclosing statement list must still satisfy the
existing explicit-return rule. Unreachable statements after a body return fail.
Existing source/token/node/expression and 64-active-block limits include while.

## OIR and verification

Each while reserves exactly three blocks: condition header, body entry, and exit.
The preheader goes to the condition header; the condition's actual final block
branches to body/exit. Falling body paths go back to the header; returning paths
have no synthetic backedge. Conditions containing calls or logical joins remain
explicit CFG fragments. Header/exit/preheader-branch origins use the full while
statement; body entry uses its source block and backedge uses its closing brace.

The verifier accepts arbitrary reachable cyclic private raw CFGs, including
irreducible graphs and arbitrary/nonzero entries. Structural bounds, IDs, types,
spans and terminators are checked before graph traversal. Unreachable blocks still
fail. Iterative Lengauer–Tarjan simple link/eval with path compression replaces
the DAG-only LCA algorithm. It uses O((B+E) log B) work and O(B+E) scratch; no source
reducibility assumption, recursion, new work cap or blocks-by-locals matrix is used.
The original algorithm reference is [Lengauer and Tarjan (1979)](https://doi.org/10.1145/357062.357071).

Peak dominator/predecessor scratch is 11B+E+1 usize cells: 31,200,008 bytes at
B=300,000/E=600,000 on a 64-bit host. Value-definition and place-initialization
records remain independently bounded by 100,000 combined slots (about 2.4 MB on
the qualified host), excluding raw IR/source, vector headers and allocator costs.
Only one function's verifier scratch is live at once.

Every value still has one globally unique static definition. Parameters are entry
definitions. Ordinary reads require definition dominance and earlier same-block
position; a call result is unavailable before its normal-return edge. Every place
has one static canonical initializer dominating each load/store. Stores do not
initialize places or define SSA values. Every two-input bool merge checks its
actual predecessor edges and edge-available values, including self/backedge inputs.
A merge reads its selected input before replacing its own result.

Repeated dynamic execution may replace a nonparameter value at its same verified
static definition, including call results and merges. Reexecuting a canonical
Initialize resets that body's reused place. It does not permit duplicate static
initializers or parameter writes. The executor never clears a whole frame on a
backedge, allocates new per-iteration frame storage, or treats a stale slot as an
alternative to the dominance proof. Fresh function activations remain independent.

## Shared operation budget

One invocation shares the existing reference ceiling of 1,000,000 fuel units
across all loops and calls. This is an abstract-operation budget, not wall-clock
time, a benchmark score, an OS sandbox, or a promise about final optimized mode.
There is no new public budget/profile flag.

Costs are unchanged: root = 1 + complete value/place slot count; each assignment,
Initialize, Store, Load, bool merge, Branch, Goto and Return = 1; call = 1 + argument
count + complete callee slot count. The callee allocation is not charged twice.
All body-local/skipped-path storage counts once per activation; executed operations
count again on every iteration. Unexecuted branches/RHSs consume no operation fuel.

Charge before the operation. Insufficient fuel gives E0601/oir-run at that next
operation's full origin and no result. Arithmetic can instead give E0604 at its
operator after its own charge succeeds. Failed RHS or failed store fuel performs
no store. For example `let mut x=0; while x<3 { x=x+1; } return x;` has nine slots
and exactly 46 fuel with the stated lowering. Its grouped-condition/grouped-RHS
variant has eleven slots and 55 fuel. All 46 lower budgets have individually
checked exact failure spans; the independent Python model checks the grouped form.

## Native admission and guarded ABI

The entire call graph remains nonrecursive, including unused functions and dead
calls. Existing inclusive caps remain: 256 functions, 64 parameters/function,
256 combined slots/function, 8,192 aggregate/live slots, 4,096 original OIR blocks,
32 call frames. Native support remains Linux x86_64 LLVM/Clang/LLD 19.1.7 O0.

Every function with an acyclic CFG and only transitively acyclic callees retains
the previous conservative static 100,000 fuel cap. Sum every block and repeated
call site exactly as before. A cyclic CFG or cyclic callee makes that static cost
unknown; summing its blocks once is never used as an execution bound. Depth and
live slots remain statically bounded through the acyclic call graph independently.
An unused expensive acyclic function is still rejected even in a guarded module.
A while whose body always returns can have an acyclic CFG and use old admission.

If any function has cyclic work, the entire emitted module is guarded. A private
stack-owned i64 counter is passed as a hidden pointer to all internal functions;
there is one counter per executable invocation, not one per loop/call. Main charges
its root allocation; each caller charges its call cost before entering the callee.
Every dynamic OIR operation compares unsigned remaining fuel before subtracting
and updating it. No wrapping fuel subtraction or unchecked arithmetic is exposed.

A bool phi must be first in its LLVM block; this pure selection precedes the
merge's fuel guard, but no effect or later operation runs before that guard.
All actual guarded predecessor edges leave a terminator-guard success block;
phi labels therefore include guard, arithmetic and call expansions. Mutable
allocas remain in the function entry prologue, including for nonzero OIR entries.

Fuel exhaustion embeds exactly the reference human E0601 diagnostic, with escaped
compile-time path and Unicode-scalar line/column, empty stdout, exit 1. Overflow
keeps E0604 and its operator origin. The existing length-delimited noreturn adapter
writes either diagnostic; write/EINTR/SIGPIPE behavior remains unchanged and a
failed diagnostic write exits 74. Embedded data requires no runtime source files,
Oxid, LLVM, JSON runtime ABI or tool access.

## Explicit guarded representation bounds

Only guarded modules acquire two additional inclusive engineering limits:
16MiB aggregate deduplicated human diagnostic bytes and 64MiB emitted LLVM UTF-8
text. Origins are keyed by failure kind/file/start/end. The human renderer streams
escaped text into a checked counting writer before allocating each diagnostic;
its format contains paths and locations, not source excerpts. Exact LLVM emission
is counted without allocating the module, then rendered only after the cap passes.
Overflow/saturation of any count cannot be admitted. Acyclic module admission is
unchanged by these guarded-only limits.

Failure is E0700/native-admission before tool discovery, temporary directories or
output writes. Inclusive/one-byte-over checks cover the actual formatting path;
100,000-instruction verified raw cyclic modules independently exercise rejection
at both default caps. These bounds constrain expanded representation, not native
instruction count, machine-code bytes, filesystem latency or total tool duration.
Very long display paths combined with many distinct operations can reach them.
Normal source while examples and resource-boundary corpora remain qualified.

## Validation boundary

Independent exhaustive/path-removal cyclic dominance oracles, raw SSA/place/call/
merge tests, actual LLVM corruption tests, exact fuel cutoffs, source-state/fuel
Python differentials, source-free ELF execution and all predecessor gates are
required. See [while validation](../docs/architecture/while-validation.md) for the
actual executed commands and counts. No O2, native recursion, ownership, escaping
references or broader roadmap completion is claimed.
