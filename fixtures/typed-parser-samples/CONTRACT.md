# Bounded scalar-function parser: admission contract

This is a proposed component layout and grammar, not an implemented parser.
Input is the existing complete token tape for at most 128 ASCII bytes. The
production parser, providers, syntax, resource caps and source-provenance rules
are unchanged. Admission must pass before broad parser implementation begins.

## Grammar

A unit contains zero or more `[pub] fn name(params) -> type { statements }` items.
Parameters are `name: type`, separated by commas without a trailing comma. Types
are one unqualified identifier or `()`. Unknown type names remain syntax.

Statements are let/let mut with optional type annotation and required initializer;
bare-name assignment; expression statement; return with optional value; if with
optional else block; while; break; and continue. Simple statements require `;`.
There are no standalone block statements, tail expressions or `else if` shorthand.
Loop placement, names, duplicates, types and main signatures are not checked here.

Expressions are booleans, decimal spelling, unit, unqualified names/calls, Group,
Negate, Not, arithmetic `+ - * / %`, comparison `== != < <= > >=`, and logical
`&& ||`. Calls accept value expressions, with no trailing comma. Precedence,
associativity, signed-literal representation and both canonical expression
nesting/tree-height checks are preserved. Decimal conversion is not performed.

Success retains exact canonical syntax fields and spans. A failure reports only
the first canonical diagnostic, including code, stage, message, primary span,
secondary spans and notes. Canonical recovery lists are outside this increment.
No partial AST is published. A component-domain refusal is a different result.

## Exact grammar-site refusals

Refusal occurs only when the running parser reaches the following site. It does
not scan later source for excluded words before handling an earlier error.
Canonical entry guards retain priority: primary depth >=64 fails before
inspecting or refusing its token family, and a prefix-depth failure occurs before
consuming that prefix. Thus 64 `!` prefixes followed by `[` yield canonical
E0400 at the bracket, not an array-domain refusal.

Malformed attempts in an excluded family are also refused; diagnostic parity is
not claimed after entering that family. Refusal records family and starter span.

| Site | Trigger | Refusal family and starter |
|---|---|---|
| Top-level item | `mod`, `use`, `struct`, exact unsupported word `enum` | module/import/record/enum, that keyword token |
| Top-level item | `pub` followed through trivia by `mod`, `struct`, `enum` | corresponding declaration family, declaration keyword (not `pub`) |
| Parameter type | `&` | reference type, ampersand token |
| Any type start | `[` | array type, opening bracket token |
| After a type name | adjacent `::` tokens | qualified type, first colon token |
| Statement start | exact unsupported word `match` | match, keyword token |
| Expression primary | `[` | array literal, opening bracket token |
| After primary name | adjacent `::` tokens | qualified value/call, first colon token |
| Statement-leading name (including field-assignment dispatch), or expression-primary name | `.` | field/index/length family, first dot token |
| After primary name | `[` | indexing, opening bracket token |
| After primary name in ordinary expression context | `{` | record literal, opening brace token |
| Call argument start | `&` | borrow argument, ampersand token |

Adjacency of `::` uses the original byte spans; `: :` is not a qualified-path
refusal. In condition-root context, `if name {` and `while name {` parse a Name
condition and then a body. Parenthesized subexpressions and call arguments use
the canonical ordinary-expression context. Postfix `[`/`.` after non-name
primaries are canonical parse errors, not newly admitted compound expressions.

All other unexpected tokens follow canonical expect/error behavior. In
particular, String/Invalid/unsupported tokens are not globally refused just
because they cannot form a valid expression. The canonical E0101 unsupported
spelling override is preserved. The ordinary-primary trailing `[`/`.` check
occurs before completing/checking the height of that primary, including after a
call, Group or Unit; it is not deferred until an enclosing statement. `&` outside parameter/argument sites is not
silently reclassified as a supported reference expression. `pub` not followed
by an admitted/excluded declaration trigger follows the canonical error path.

## Fixed row representation

Logical AST capacity is 128 rows. Candidate physical columns are either 128 or 129
cells; the latter reuses the existing zero-array helper, and row 129 is permanently
unused and zero. There are seven i32 columns: kind, span, a, b, c, d, next.
Spans use `start + 256 * end`, with checked 0 <= start <= end <= 128. References
are one-based, zero meaning absent. No source text is copied into row storage.

| Row | span | a | b | c | d | next |
|---|---|---|---|---|---|---|
| Function | name | pub token | parameter head | result type | body block | next item |
| Parameter | name | type row | 0 | 0 | 0 | next parameter |
| TypeName/TypeUnit | complete type | name token/0 | 0 | 0 | 0 | 0 |
| Block | complete block | closing brace token | statement head | 0 | 0 | 0 |
| Let/LetMut | complete statement | name token | optional type | initializer | 0 | next statement |
| Assign | complete statement | name token | equals token | value | 0 | next statement |
| ExprStmt/Return | complete statement | value/optional value | 0 | 0 | 0 | next statement |
| Break/Continue | complete statement | 0 | 0 | 0 | 0 | next statement |
| If | complete statement | condition | then block | optional else block | 0 | next statement |
| While | complete statement | condition | body block | 0 | 0 | next statement |
| Number | complete expression | digit token | negative flag | 0 | height | optional next argument |
| BoolTrue/BoolFalse/Unit | complete expression | 0 | 0 | 0 | height | optional next argument |
| Name | complete expression | name token | 0 | 0 | height | optional next argument |
| Call | complete expression | callee token | argument head | 0 | height | optional next argument |
| Group | complete expression | child | 0 | 0 | height | optional next argument |
| Negate/Not | complete expression | child | operator token | 0 | height | optional next argument |
| Each binary operator | complete expression | left | right | operator token | height | optional next argument |

Temporary list tails may use a not-yet-final field, but every completed row must
restore all specified zero fields before validation/output. The token tape makes
all token references exact spans and raw spelling references. Function end is
its body block's closing token. Canonical global expression IDs are projected by
ordered postorder traversal, and canonical block IDs by per-function preorder;
physical unified-row allocation IDs do not claim to be canonical IDs.

## Token charging and control frames

Each allocated row has a unique charge token: Function=`fn`, Block=`{`, Parameter=
its name, Type=its name or opening `(`, statement=its keyword/assignment `=`/
expression-statement `;`, expression=its primary/operator/opening `(`. These are
disjoint by grammar position. A Call charges its callee token without a separate
Name row. A signed Number charges its digit token; its sign consumes no separate
row. Group and Unit each charge their own opening `(`. Error exits publish no AST.
Reserve a row only after consuming its charge token. In particular, do not
reserve an expression-statement row before its still-unseen semicolon: the
already charged Block frame awaits the expression, then consumes `;`, reserves
the statement row and links it. The expression root supplies its starting span.
Similarly, reserve Function only after `fn`, Parameter after its name and Block
after `{`. Signed-number lookahead chooses one Number row, never an extra unary
row for the same literal. These rules apply before failure as well as on success.
Thus at most 128 nontrivia input tokens imply at most 128 allocated rows.

Use an explicit driver plus a continuation bank of state/node/aux columns, 129
cells each. Every pending continuation must own a distinct reserved AST row;
there is at most one additional root driver frame. No separate frame is pushed
for empty precedence tiers. Pending binary/unary/group/call rows hold expression
continuations; pending function/block/statement rows hold grammar continuations.
The current reduced value, token cursor and failure facts are fixed driver
scalars. A parent row is suspended once, resumed in place, and cannot be pushed
again while suspended. Returning a child fills the parent row, reuses that frame
for its next phase, or pops it; it never adds a second frame for the same row.

Expression continuation aux must retain canonical depth, ordinary/condition
context and a bounded precedence threshold. A candidate encoding is depth +
128*context + 256*threshold, with depth 0..64/context 0..1/threshold 0..7. Higher
control modes live in the explicit state tag. Lists use row links, not duplicate
operand/argument/statement arrays. Preserve the separate computed tree height.

The capacity argument is conditional on those state transitions. Before parser
implementation, review the complete finite transition inventory and use the
carrier probe to exercise pushes, in-place resumes, reductions, list linking,
maximum rows, exhaustion checks and complete output validation. Acyclic helpers
are mandatory: current native admission rejects cyclic call graphs. No runtime
recursion assumption or raised compiler limit may replace this requirement.

## Finite continuation inventory for the admission probe

The parser driver has fixed scalar registers for cursor, current reduced value,
expression depth/context/precedence threshold, row count, stack top and failure.
Frame tags encode the following resumptions; their row is already reserved.
A state change replaces the tag of that same frame, rather than pushing again.

| Frame role | States and actions | Child frame / completion |
|---|---|---|
| Root (only rowless frame) | await item or EOF | consume `fn`, reserve/push Function; completed function linked to root item list |
| Function | name, parameters, result, body, end | parameter/type atoms parsed by acyclic nonrecursive helpers; consume `{`, reserve/push Block; complete after returned body |
| Block | next statement, expression-statement value, closing brace | keyword/assignment statements reserve their charged row and push that row if a child is needed; expression statement reuses Block frame until `;` is consumed |
| Let / Assign / Return | await value, require semicolon | drive expression in ordinary context/depth0; link result, consume `;`, complete/pop; bare Return completes without expression |
| If | await condition, then body, optional else body, finish | condition context/depth0; enforce block-depth gate before body; each `{` reserves/pushes one Block; else requires `{` |
| While | await condition, body, finish | condition context/depth0; enforce block-depth gate; one child Block |
| Prefix Negate / Not | await operand | consumed operator owns row; depth increment checked; on return link child, check height, complete/pop |
| Group | await inner expression, require `)` | consumed `(` owns row; ordinary context/depth+1/floor0; immediate `)` changes row to Unit without an extra frame |
| Call | argument value, comma-or-close, finish | callee owns row; ordinary context/depth+1/floor0; link each argument through next, reuse Call frame; close clears temporary tail and completes |
| Binary arithmetic/logical | await right operand, reduce | consumed operator owns row and left is stored; right threshold implements left associativity; link right, check height, complete/pop |
| Binary comparison | await right sum, reject chain, reduce | after the right sum, reject a second comparison token before constructing/checking comparison height; then link/check/complete |

Break/Continue and leaf expressions complete without an additional continuation.
Type/parameter helper calls cannot recurse or drive nested expressions. Simple
statement syntax is recognized before consuming an expression, so an assignment
never creates a redundant Name expression for its target. Operator recognition
reserves the row only after consuming the operator. Signed decimal minus is
handled as one literal before deciding to create a prefix frame.

The expression driver switches between expecting a primary and reducing a value.
A caller's existing frame supplies the expression boundary; there is no separate
expression-root frame. Binary and prefix frames save the prior expression
registers in aux before changing them; completion restores them. Parent rows
remain suspended exactly once. Completed rows can be linked but never pushed as
fresh continuations. List-head/tail fields are reused only while that row's frame
is active, and completed rows zero every scratch-only field.

The capacity proof is inductive: root uses one frame; a push consumes a previously
uncharged token and reserves a new row; changing a phase does not change frame
count; pop reduces it. There are no other pushes. Expression-statement allocation
after semicolon does not push a frame. Thus stack length never exceeds reserved
rows+1, including malformed input, and never exceeds129. A probe must check this
invariant on every transition and demonstrate maximum push/pop and repeated
in-place resume/list-link sequences. It is a control-storage admission proof,
not yet evidence that all real parser transitions implement the grammar.
