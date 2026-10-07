# Complete bounded scalar resolution, pending typing

`fixtures/typed-lexer-samples/resolver_main.ox` is a separate source-to-resolution
component over the existing 128-byte scalar parser grammar. The permanent
`static_main.ox` stress probe remains available independently.

The resolver performs global/signature preflight, canonical unknown-type
priority, every function body, lexical scope and local identity assignment,
checked decimal conversion and nearest-loop binding. It visits unreachable and
untaken syntax before any future type checking. Function names exist before all
bodies; initializers precede their new bindings, and assignment/call targets
precede their children. Local IDs are monotone per function, including across
sibling scopes; popping a scope never recycles an ID.

Success emits the complete OPA1 frame followed by a tag-2 STF1 frame containing
resolution facts and an all-zero semantic column. The host observation says
`resolved`, `phase: resolve`, `typing: pending`. It contains actual resolved HIR,
not a TypedProgram, execution artifact or source-association witness. Real first
resolution failures use the specified tag-1 diagnostic header and omit columns.

Traversal uses the existing three-field State, frame bank and active-local bank.
Frames encode checked row/phase pairs. Blocks place distinct scope-marker rows in
the active-local bank; lookup skips markers, and scope exit pops through its own
marker. All banks are cleared between functions. Diagnostic handoff happens only
after traversal ends and uses the same State with bounded kind/spans.

The first complete implementation passed typed checking but failed native
admission because one dispatcher exceeded the 256 scalar-slot function limit.
A small acyclic leaf handler split preserves traversal/error order and passes the
unchanged gates. The dispatcher is now 253/256 scalar slots. Whole-program I is
7,435, W is 3,961, with 103 functions, 1,635 blocks and 76,648 explicit native
bytes including the process wrapper. Only 757 I items remain. These measurements
do not establish that complete typing/flow will fit.

Fifty authored inputs compare complete canonical resolved HIR or the first exact
resolution diagnostic in both reference and native modes: 100 comparisons pass.
The matrix covers declaration order, forward recursion, public routing,
unknown-type/annotation precedence, scope exit and identity restart, all
expression families, target/initializer/argument ordering, unreachable and
short-circuit syntax, MIN/MAX/range cases, and nested loop transfers. The separate
projection has 16 focused malformed/structural controls; 27 inherited parser
checks also pass. Canonical resolution mode passes 14 focused tests with 91 total
observations; all 65 default typed outputs remain byte-identical to its earlier
reviewed observer.

No production compiler behavior or provider selection changes here. Full typing
and flow remain a separately measured stage; any native gate failure must remain
visible and stop expansion before qualification.
