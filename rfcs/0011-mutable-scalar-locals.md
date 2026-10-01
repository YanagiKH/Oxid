# RFC 0011: initialized mutable scalar locals

Status: accepted for restricted experimental implementation on 2026-10-01.
The user approved initialized `let mut`, fixed scalar types, statement assignment,
immutable ordinary lets/parameters, RHS-before-write and mutation in existing if
branches. This extends [RFC 0010](0010-boolean-logical-operators.md) without loops,
borrows, a new value category, ownership certification or a completed milestone.

## Source contract

`let mut name (: type)? = expression;` introduces a mutable local with an immediate
initializer. Its inferred or annotated bool, i32 or unit type remains fixed.
`name = expression;` replaces an existing mutable local; it is a statement and
produces no value. Only bare names are assignable. Parenthesized names, call
results, fields, indices, chained assignments, assignment expressions, compound
operators and mutable parameter syntax remain unavailable.

Ordinary `let` and function parameters remain immutable. Existing lexical rules
apply equally: no duplicate active name, no active-ancestor shadowing and no
shadowing a function. Distinct sibling/closed scopes may reuse a spelling with
new identities. Initializers cannot see the binding being introduced; assignment
does not introduce a binding or extend its scope. All scalar reads and arguments
are value snapshots. Reassigning a place does not change an earlier copied value
or a caller's state through a callee's independent local.

Evaluate the RHS fully, once, in existing source order before replacing the
place. Calls, checked arithmetic and logical joins complete before the store.
A failed RHS or exhausted store fuel performs no store; there is no rollback of
previously completed instructions. A false if arm and short-circuited logical RHS
do not execute. Both arms, all RHSs and unused functions are still resolved and
type checked; the entire call graph still participates in native admission.

`mut` is a reserved token. Bare-name assignment uses bounded token lookahead
past trivia and does not add an expression precedence level. Invalid positions
for newly recognized `mut` and `=` produce E0100 syntax errors. Unknown targets,
including function names used as targets, produce E0200/resolve at the target.
Assigning an immutable binding produces E0304/type at the target with a declaration
label. A fixed-type mismatch produces E0300/type at the RHS with a declaration
label. Existing phase ordering still governs multiple static errors.

## Typed places and immutable values

HIR local metadata carries mutability separately from type and lexical identity.
HIR assignment carries a resolved local ID, target-name origin, `=` origin and
ordered RHS expression. Typed construction establishes mutability and exact RHS
type before lowering. Parameter metadata is always immutable.

OIR uses separate declaration tables and nominal IDs for immutable values
(`LocalId`) and mutable places (`PlaceId`). A `Place` contains an ID and use span;
a `PlaceDecl` contains the closed scalar type and declaration span. The function's
parameter prefix belongs only to the value table. A source mutable binding uses
one place rather than an extra value binding slot.

Each ordinary block contains an ordered `Statement` sequence:

- `Assign` defines one immutable value from an existing Rvalue
- `Initialize { place, value }` canonically initializes one place
- `Store { place, value }` replaces that place's scalar, retaining its `=` origin
- `Load(place)` is an Rvalue that snapshots the current place into a new immutable
  assignment destination

Initialization/store inputs are ordinary immutable value operands. Stores have
no value destination. Calls and bool merges still define unique value IDs, never
places; source mutable reads always lower to explicit loads. Root-directed
expression lowering completes the entire initializer/RHS, including all actual
call/checked/logical continuations, before appending the state instruction.
No opposite-arm SSA writes or generic phi machinery is introduced.

## Independent verification

Preflight counts values and places together under the existing aggregate slot
ceiling. Before any CFG walk, validate every place declaration, reference, type,
span, initialization/store input, load result and store operator origin, including
unreachable blocks. There are no source-name or producer-trust shortcuts.

Each place must have exactly one canonical initialization, even if unused. The
initialization table is distinct from the immutable definition table. An init's
RHS must already have an available SSA definition. Every load or store requires
an earlier initialization in the same block or a dominating initialization in
another block. Opposite-arm initialization, self-initialization and pre-init
access fail. A store never counts as an initialization or SSA definition.

The existing iterative acyclic reachability/topological/dominator proof is reused.
Block IDs need not be topological or start at zero. All unique value definition,
call-continuation and bool-merge edge rules remain. A call result can be stored
only after its normal continuation, never before its call. No blocks-by-places
matrix, per-block set cloning, loop fixed point or production path enumeration
is added. Raw tests compare initialization availability to independent path
removal over all bounded small DAG fixtures, including reversed block IDs.

## Execution, limits and native lowering

Reference frames own separate optional scalar value and place arrays. A place
initializer writes once, a store requires an initialized place, and a load
copies its current value. Type/ID/initialization checks remain in the consumer.
Each init, store and load costs one fuel, charged before reading/writing. Complete
value+place storage counts toward root/call allocation fuel and live-slot caps,
including places declared in unchosen arms. Returning releases both arrays.

`fn main() -> i32 { let mut x = 1; x = 2; return x; }` needs four slots and eleven
fuel: root entry 1, allocation 4, literal/init/literal/store/load 5, return 1.
Existing programs without mutable places retain their previous costs.

For P parameters, L lets including M mutable lets, X expressions, C calls,
A reassignment statements, Rb bare returns, F functions, I if statements and
S logical expressions:

- value slots = P + (L - M) + X + Rb; place slots = M
- combined slots = P + L + X + Rb
- instructions, including bool merges = X - C + L + A + Rb
- blocks <= F + C + 3I + 2S; edges <= 2 * blocks

The 100,000 combined slot/instruction ceilings, 300,000 OIR block ceiling and all
source/parser/reference limits stay fixed. Separate definition/initialization
tables together have at most 100,000 entries, so verifier scratch/time remain
within the prior asymptotic bounds.

Native admission accepts only verified scalar init/store/load in addition to the
existing allowlist. Its 256 slots/function, 8,192 aggregate/live slots and 100,000
conservative reference-fuel bound include places and each state instruction.
Untaken work remains conservatively counted; recursion remains rejected. The
acyclic CFG still justifies counting each block once, including both branch arms.

LLVM allocates each place privately in the actual function entry prologue before
branching to the OIR entry, even when that entry has a nonzero ID. Types are i1,
i32 and the existing i8 zero representation for unit. Explicit typed load/store
instructions preserve statement order, while immutable values keep their SSA
names. Checked-overflow blocks and logical phi predecessor labels remain as
before. No uninitialized load is admitted. Pointers are an internal backend
representation, never source values or an FFI/aliasing feature.

Native O0 Linux x86_64 remains the qualified target. The adapter ABI, exact E0604
stderr/exit convention, source-path data escaping, tool validation, atomic
no-clobber output and libc dependency are unchanged. No O2/LTO, native loop,
borrow safety, heap ownership or cross-platform native claim follows.

## Acceptance and evidence

Public tests cover fixed types, all scalar snapshots, sequential/branch state,
scopes, selected RHSs, independent calls, static negatives and exact origins.
Raw tests cover malformed places, canonical initialization, dominance, SSA
separation, reordered CFGs and aggregate boundaries. Reference-only event tests
prove store order and absence of a write after RHS/fuel failure. Mandatory raw
LLVM tests exercise typed private allocas and nonzero-entry CFGs in both profiles.
An independent tagged Python state model compares source evaluation, bounded
reference results and actual standalone native artifacts, with seeded programs,
first-error origins and exact native limits. All prior corpora remain gates.

See [mutable-local validation](../docs/architecture/mutable-locals-validation.md)
for fresh results. Future cyclic flow/runtime-fuel and non-Copy ownership policies
remain separate design decisions.
