# RFC 0023: bounded nominal enums and consuming match

Status: accepted bounded experimental implementation contract, 2026-10-05.
Implementation and qualification are pending.
Design base: main `c37a09f332ad68ab401f1c6936e2e17afa430e90`.

## Boundary

Add nonempty nominal enums with 1..256 unique variants, each nullary or carrying
exactly one bool/i32/() payload. Example:

```oxid
enum Token { Integer(i32), Plus, End, Invalid(i32) }
fn consume(token: Token) -> i32 {
    match token {
        Token::Integer(value) => { return value; },
        Token::Plus => { return 1; },
        Token::End => { return 0; },
        Token::Invalid(code) => { return 0 - code; },
    }
}
```

Construct with `Token::Plus` or `Token::Integer(expression)`. Nullary variants
reject parentheses; payload variants require exactly one expression, including
`Token::Unit(())` for a `Unit(())` declaration. Exact types, no coercions.
Enum values are whole-value move-only locals, mutable replacement destinations,
by-value parameters and by-value results. `main` still returns bool/i32/() only.
Matching requires a bare available named local or by-value parameter. It consumes
the owner once before the selected arm and gives only that arm its immutable,
arm-local scalar binding. No shadowing; arm bindings follow existing lexical
scope rules. Every variant appears once, order arbitrary, even for dead arms.
Missing/duplicate/foreign variants and arity/type mismatch reject statically.
Statement-only match arms use braced blocks; commas separate arms and a trailing
comma is optional. All-path return, break/continue, conservative ownership joins,
and loop backedge rules are inherited. A consumed scrutinee is unavailable on
all outgoing edges unless explicitly reinitialized under existing mutable-local
rules. Only reachable continuing arms participate in a post-match join.

Enum type names share the existing type namespace with records and builtins.
Variants are visible wherever their type is visible, with no separate `pub`.
Resolve the type prefix through the shared declaration index and existing type
aliases/absolute paths, then resolve its variant; no global variant namespace
or new import form. Enum syntax uses the existing owned source/runtime path.

No enum borrows, fields, arrays/slices, aggregate/recursive payloads, stored
references, partial moves, match expressions, guards, wildcard/alternative/nested
patterns, custom tags, integer tag access, equality, printing, new public ABI,
artifact revision, legacy changes, production provider or self-hosting claim.

## Resource and representation acceptance gate

Enums plus records share the 4096 aggregate declaration cap. Variants plus record
fields share the 65536 aggregate member cap. Record fields retain the existing
1024 per-record cap; enum variants have the tighter 256 cap. Existing 100000
parser node, 64 expression/block depth, path and all byte/work ceilings remain.
Each declaration, variant, constructor, match statement and arm is counted before
retained allocation; arm bodies also retain ordinary statement/block accounting.
Checked declaration tables retain 8 MiB, summed padded layouts 1 MiB, reference
200000 expanded cells/16 MiB and native 8192 cells/1 MiB limits. Additional
metadata may cause earlier admission failure; no cap is raised.

### Measured representation decision

Use `AggregateTy::Enum(EnumId)` and its compact checked owner-slot counterpart;
keep separate nominal enum/variant identities, with declaration order stable.
A small x86_64 rustc 1.99.0 candidate-layout probe measured AggregateTy-shaped
carriers at 16 bytes before/after, compact slots at 8 bytes before/after. A u32 tag
plus four payload bytes is 8 bytes/alignment 4. A u8 tag plus explicit padding
still needs eight bytes once aligned for i32, so use the simpler u32 tag. These
are candidate measurements, not a substitute for actual enclosing-carrier tests.
Actual AST/HIR/raw/checked/plan sizes must be measured before executable admission.

Every enum has private size 8, alignment 4 and logical width 2: a four-byte
little-endian declaration-order tag at offset 0 and four bytes reserved at offset
4 for the active scalar. Bool/unit use their ordinary one-byte encoding; i32
uses four. Nullary variants have no payload read/write requirement. Zeroing
storage is allowed; no operation depends on inactive bytes or padding. Tags are
internal, not scalar source values. The checked variant table determines payload
type; a caller-supplied tag/type/offset is never authority. Sum layout is constant
even for nullary-only enums, avoiding representation changes as variants evolve.

Whole transfers first validate the incoming value's tag and active payload, then
write only the tag and active payload and transition ownership. Replace may
overwrite an available or moved mutable destination without reading its old tag,
payload or inactive bytes; it does not implicitly Discard the overwritten value.
This permits reinitialization after a consuming match without a runtime sidecar.
Explicit Discard likewise
validates the available value before consuming it. StorageEnd and frame teardown
end slot lifetimes without exposing or transferring their values: they are
payload-free and never inspect tags or payload bytes of uninitialized, moved or
dead storage. No runtime owner-state sidecar is added to the native backend. Never call the static aggregate
leaf iterator on an enum. Construction, move, replacement, argument staging,
callee activation, return and teardown all need explicit sum handling. Native
transfers branch on checked tags, never load the whole union as a typed payload,
and retain the existing private owned calling convention. No enum borrow is
valid even though enum storage uses the existing owner machinery.

### Raw control-flow proof and exact fuel

Keep the existing two-successor CFG contract. Each match has a bounded raw
`MatchDecl` containing its source owner and a nonempty arm list in written order.
Each arm names a nominal variant, one dispatch block and one distinct arm-entry
block. Dispatch blocks form a canonical binary chain: every nonfinal dispatch
branches on its variant to its entry, or to the next dispatch; the final dispatch
has one successor and verifies the remaining tag. Invalid raw/runtime tags fail
closed before any active payload read or ownership mutation, including at the
final dispatch and all whole-value transfers. Every arm entry begins with
`ConsumeVariant { match, arm, destination? }`, which atomically validates and
copies the active scalar to its immutable local (when any), then marks the source
owner moved. There is no standalone payload-read or source-visible tag operation.

Independent raw shape checking proves exact same-enum exhaustive coverage,
unique variants, unique canonical dispatch/consume sites, correct payload local
type/arity, no alternative predecessor to intermediate dispatch or arm-entry
blocks (counting edge multiplicity), no extra instructions in intermediate
dispatch blocks, and exact branch links. Function entry is forbidden as an
intermediate dispatch or arm entry: the implicit function-entry edge is not
established by predecessor counting. The first dispatch may follow ordinary
preceding statements. A dispatch block cannot double as another match's arm entry. Each descriptor is used once;
all raw blocks, including unreachable blocks, are shape-checked. Every reachable
consume is therefore entered only from its matching dispatch. Existing scalar
definition/dominance checks see ConsumeVariant's destination as an ordinary
single definition, and existing whole-owner flow proves availability through the
dispatch chain and one consumption on every selected arm. No new runtime sidecar
or generalized CFG/scalar-edge-definition proof is needed.

All dispatches cost 1 before reading the tag. ConsumeVariant costs `1 + width =
3` before payload extraction, binding or owner transition. Selecting zero-based
written arm k therefore costs exactly `k + 1 + 3` for dispatch and consumption;
arm-body operations add their existing costs. A nullary arm has the same cost.
Fuel failure at a test or consume causes no payload exposure/arm-body effect or
unpaid consumption. The dispatch and consume diagnostic origin is the match
statement; payload-expression failure keeps its own existing origin.

ConstructEnum evaluates its sole scalar expression once first, then costs
`1 + width = 3` before any value becomes initialized. Whole MoveInitialize,
PrepareOwned, Discard and StorageEnd cost 3 for an enum; Replace costs
`1 + 2*width = 5`. StorageLive remains 1. Invoke/return/frame activation keep their
existing formulas, substituting enum width 2 where an owned width is used.
These are operation costs: the normal source temporary/staging/cleanup operations
remain separately charged. No unchanged opcode or program acquires enum costs.

### Allocation/work accounting

Count and charge every new retained AST declaration/member/constructor/arm
carrier and vector before reserve, including formatter traversal storage. The
shared project declaration index owns enum rows/variant rows and resolves type
prefixes; its retained/scratch/work caps remain 32 MiB/16 MiB/256 million. Source
HIR declarations, bindings, match arms, work stacks, lowering descriptors and
expression caches use the current independently observed source allocator.

Raw programs charge enum declarations/members and enclosing growth; each match
charges its descriptor, complete arm vector, dispatch blocks and consume
instructions in both producer inventory and independently recomputed raw budget.
Descriptor/arm work is charged against existing event/work budgets (100000 events,
100 million raw work), metadata/scratch remain 32 MiB each. Canonical-site and
predecessor validation scratch is preflighted and bounded, without a quadratic
unmetered scan. Existing `edges <= 2*blocks` remains applicable. Consumer plans
charge all added retained descriptors/scratch by actual sizes, with 32 MiB plan
cap unchanged. Native tag-dispatch text, active-scalar transfer cases and traversal
work count against existing emission/time admission before output allocation.
The fixed eight-byte enum payload and logical width two enter every frame,
reference-machine and native-arena budget. Independent allocator failure sweeps
and below/at/above cap controls cover new nested vectors and enclosing growth.

## Pilot and acceptance evidence

A streaming scanner over existing &[i32] character codes and separate &mut Cursor
returns Integer/Plus/End/Invalid tokens; a consumer exhaustively consumes them.
Decimal accumulation uses existing checked i32 arithmetic: overflow produces the
ordinary arithmetic failure, never a hidden lexical fallback or wrapped token.
Use ordinary, multi-digit, whitespace, empty/end and invalid-input cases with
independently calculated expected results. This is a compiler-component example
using hardcoded integer-code input, not production compiler dispatch/self-hosting.

Required: positive/compile-fail source, formatting, project visibility, malformed
raw and dominance/ownership, active-payload/padding, poisoned inactive/moved
storage teardown, every-fuel-boundary and
allocation-limit controls; source/reference/source-free LLVM 19.1.7 O0 Linux
x86_64 parity; unchanged-program regressions; independent review; ordinary suites,
current-source qualification and green exact-head hosted CI as separate gates.

Acceptance decision: ACCEPTED 2026-10-05.
Acceptance explicitly requires invalid tags to fail closed before payload reads
or ownership mutation, including final dispatch; checked i32 overflow in the
pilot; documented written-arm-order fuel dependence; and actual enclosing-carrier
measurement plus complete descriptor admission before enabling consumers.
This decision authorizes implementation, not a completed or qualified feature.
