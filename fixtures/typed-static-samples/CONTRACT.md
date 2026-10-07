# Bounded scalar static frontend: precursor contract

Status: the isolated carrier/control precursor and scalar resolution stage are
implemented. Complete typing and provider activation are not enabled. Input remains
one source of at most 128 ASCII bytes, with the existing lexer and scalar parser.

## Semantic endpoint

The intended endpoint is complete scalar resolution followed by scalar typing:
exact signatures, bindings, literal values, HIR references, types and block flows,
or the first canonical static diagnostic. Resolution of the entire program must
finish before any typing begins. Library checking accepts empty input, recursion,
no main, and parameterized main. Entry admission, OIR, execution, constant folding,
runtime arithmetic errors and production source-association authority are later
concerns. No partial facts are published as a typed result after failure.

Unknown simple type names select the actual public owned-resolution route. Since
this grammar has no nominal declarations, that route necessarily fails before
body resolution. Preserve its diagnostic-only schedule: all global declaration
conflicts first; functions in order; parameter types, result, then let annotations
in canonical block-vector order. Unknown types use E0202/resolve and the existing
64-byte name-display rule. No ownership semantics are added by this branch.

Primitive-only inputs use scalar resolution and typing. A public function selects
project syntax: the canonical observation must use the genuine project loader
and project source owner, with exact source bytes and identity. The original
single-source adapter is only valid when project syntax is absent. Neither route
may manufacture declaration-index or source-association authority. Names are exact original
source spellings. All functions exist before body resolution; active locals and
parameters cannot shadow one another or any function. Let initializers resolve
before their new binding; assignment/call target lookup precedes their children.
Locals and HIR expression IDs restart per function; DefIds follow function order.
Parameters receive LocalIds first, followed by depth-first lets (then before
else); IDs are never recycled after scope exit. Expressions use completed
postorder within each function, retaining Group nodes, while statement roots
follow depth-first traversal. Blocks use per-function preorder. These canonical
IDs are distinct from physical AST row references. The nearest while-body block
identifies loop transfers. Resolution includes
unreachable statements and both arms. Literal conversion is exact checked i32,
including MIN and leading zeroes; range errors precede all type checking.

Typing checks a statement's reachability before its expressions, then expression
children before parent operand/arity/mutability constraints. Let annotation and
assignment type equality are exact. All functions require an explicit return on
all conservative paths, including unit functions. While retains fallthrough even
for literal true. Flow bits are F=1, R=2, B=4, C=8; branches union exits, sequencing
keeps the first summary without F and unions the next summary when F is present
(otherwise it keeps the first summary), and while consumes its own B/C exits.

## Facts indexed by immutable AST row

Keep the existing OPA1 rows, source and token tape. Two new 129-cell i32 columns
hold facts; the last cell remains zero. Physical row references are one-based.
The role is determined by the existing AST kind, not by a tagged dynamic value.

| AST row | Resolution column | Semantic column |
| --- | --- | --- |
| Function | DefId + 1 | result type |
| Parameter / Let / LetMut | LocalId + 1 | fixed local type |
| TypeName / TypeUnit | primitive type | 0 |
| Block | 0 | complete F/R/B/C mask |
| Assign | referenced declaration row | 0 |
| Break / Continue | nearest while-body Block row | 0 |
| Number | exact signed i32 value | i32 type |
| Name | referenced Parameter/Let/LetMut row | fixed type |
| Call | referenced Function row | result type |
| Bool / Unit / Group / unary / binary | 0 | inferred type |
| Other statements | 0 | 0 |

Type codes are bool=1, i32=2, unit=3; zero is absent only where specified. On static
success all active expression/local/type roles are populated, IDs are dense in
their proper namespace, references have the required kind/function ownership,
root block flows equal R, and all inactive rows/unused fields are zero. Number
zero and MIN are values, not absent/sentinel codes. AST and observed columns must
project complete HIR facts; the host must not fill missing candidate bindings,
values, types or flows by recomputing semantics.

## Observation framing

This is a bounded test/component observation, not a public compiler ABI.
Retain the complete existing OPA1 observation first. A lexical/parser failure is
its existing 11-byte frame with no static suffix. Syntax success is the existing
1,559-byte OPA1 frame followed by an STF1 suffix. This keeps syntax and semantic
acceptance distinct without inventing an alternate AST input protocol.

The suffix header is exactly 16 bytes:

| Byte | Field |
| --- | --- |
| 0..3 | ASCII STF1 |
| 4 | tag: 0 static success, 1 static diagnostic, 2 precursor probe only |
| 5 | AST row count, matching OPA1 |
| 6 | diagnostic kind below, otherwise 0 |
| 7..8 | primary start/end |
| 9..10 | secondary start/end |
| 11 | secondary label: 0 none, 1 first declared, 2 function declared, 3 binding declared, 4 immutable binding declared |
| 12..13 | expected/actual primitive type, otherwise 0 |
| 14..15 | expected/actual argument count, otherwise 0 |

Static success appends the two columns in the same byte-plane order as OPA1:
resolution first, then semantic, with planes 0 through 3 and cells 0 through 128
within each plane. The combined offsets are header 1,559, resolution 1,575,
semantic 2,091, and exact end 2,607. Each column has four planes of 129 bytes,
with signed i32 encoded as exact little-
endian two's-complement bits. Total combined success size is 2,607 bytes. All
success diagnostic fields are zero. A static diagnostic appends no columns:
total combined size is 1,575 bytes. Header fields unused by its diagnostic kind
are zero. Secondary labels are ordered singleton-or-empty; notes are empty in
this bounded canonical domain. Source spans are half-open and bounded by input.

Diagnostic kinds bind exact canonical code/stage/message templates:

1. E0200/resolve: `` unknown local `{name}` ``
2. E0200/resolve: `` unknown direct function `{name}` ``
3. E0201/resolve: `duplicate binding; shadowing is unavailable in typed-preview`
4. E0202/resolve: `` unknown type `{name}` `` (public owned-name display rule)
5. E0203/resolve: `decimal literal is outside the i32 range [-2147483648, 2147483647]`
6. E0204/resolve: `` `break` requires an enclosing while in the same function ``
7. E0204/resolve: `` `continue` requires an enclosing while in the same function ``
8. E0300/type: `type mismatch: expected {expected}, found {actual}`
9. E0300/type: `equality requires i32 or bool operands, found ()`
10. E0301/type: `wrong argument count: expected {expected}, found {actual}`
11. E0302/type: `function requires an explicit terminal return`
12. E0303/type: `statement after terminal return is unavailable in typed-preview`
13. E0303/type: `statement after terminal control transfer is unavailable in typed-preview`
14. E0304/type: `assignment requires a mutable local`

| Kind | Primary | Secondary label and span | Nonzero payload fields |
| --- | --- | --- | --- |
| 1, unknown local | Missing Name identifier or assignment target identifier | None | None |
| 2, unknown direct function | Callee identifier, not whole call | None | None |
| 3, duplicate binding | Later declaration's name | Label 1, first conflicting declaration's name | None |
| 4, unknown type | Complete TypeName identifier | None | None |
| 5, i32 range | Complete Number expression, including literal minus/trivia | None | None |
| 6, break placement | Whole break statement, including semicolon | None | None |
| 7, continue placement | Whole continue statement, including semicolon | None | None |
| 8, ordinary mismatch | Failing operand, argument, initializer, RHS, returned value, or condition; full `return;` when value absent | Label 2 on a call argument mismatch; label 3 on let/assignment mismatch; otherwise none | Expected and actual primitive type codes |
| 9, unit equality | Left operand expression | None | None; the message already fixes unit |
| 10, arity | Whole call expression | Label 2, called function name | Expected and actual argument counts, including valid zero counts |
| 11, missing terminal return | Function closing-brace span | None | None |
| 12, after terminal return | Whole next statement | None | None |
| 13, after terminal control transfer | Whole next statement | None | None |
| 14, immutable assignment | Assignment target identifier | Label 4, declaration name | None |

For kind 8, expected/actual codes are each in 1..3 and differ. For kind 10, counts must fit the actual bounded AST and differ; zero is a legitimate count. For all other kinds bytes 12..15 are zero. If label is absent, bytes 9..11 are zero. The primary must denote the context listed above rather than merely be some in-bounds span. Named secondary labels must match their exact canonical text. Kind 8's allowed secondary label depends on the actual failing constraint; it must not accept an arbitrary label among 0/2/3.

The quoted templates, source-derived name spelling and context-specific spans
are checked against the unchanged canonical observer before semantic activation.
Type displays are exactly `bool`, `i32` and `()`. Secondary labels are exactly
`first declared here`, `function declared here`, `binding declared here` and
`immutable binding declared here` for labels 1 through 4. Process status 0 means
a complete observation, not semantic acceptance; inherited transport statuses
64/74 and internal failure 70 remain distinct. Incomplete output is rejected.
The precursor must not emit tags 0 or 1. Its only successful suffix is tag 2,
with zero diagnostic fields and full-width synthetic probe columns. Those
columns need not obey semantic row roles; a semantic consumer rejects tag 2
before reading them as typed facts. Their last cell remains zero. Probe success
is never static acceptance, a TypedProgram or a source-association witness.

## Resolution-only stage

The separate `resolver_main.ox` root resolves the complete bounded program before
any typing. Its successful suffix remains tag 2, with the complete resolution
column, all semantic cells zero, and no diagnostic fields. Its host observation
is explicitly `resolved`, phase `resolve`, typing `pending`; it cannot be consumed
as complete static success. A real first resolution failure uses tag 1 with kinds
1 through 7 and no fact columns. The permanent synthetic probe root is unchanged.

The canonical resolution observer returns actual HIR before invoking the checker.
It preserves both genuine scalar/project source routes and the diagnostic-only
unknown-type route. The projection derives only structural IDs and copies observed
binding targets and literal values; it does not resolve names, convert decimals,
infer types or compute expected flow. See [RESOLUTION.md](RESOLUTION.md).

## Carrier/control precursor

Add the two fact columns and a separate 129-cell active-local stack. Reuse the
existing parser frame buffer only after successful parser completion: mode 9,
stack 1, root marker in slot 0 and all other slots clear. Consume/reset that root
marker before the new phase; no parser continuation or loan remains live.
Use a small explicit scalar State record. Do not overwrite immutable AST rows.

Exercise all 128 usable fact/local/continuation positions, clearing popped and
inactive storage; repeated in-place resumes; actual bounded source-name lookup;
exact i32 MIN/MAX and out-of-range decimal controls; and representative four-bit
flow operations. Serialize both complete fact columns, including signed values,
and reject malformed/boundary conditions without overflowing the component.
No runtime recursion, callback framework, dynamic table, source fusion, new
input framing or cap increase is authorized to make the probe fit.

Measure the whole actual combined program, including constructor/return/binding
copies and output helpers. The initial measured tag-2 carrier had I = 7,404, W = 3,645,
103 functions, 1,747 blocks and 75,020 explicit native bytes including the process
wrapper. That left 788 I items. The exact-table keyword successor has I = 6,536,
W = 3,925, 96 functions, 1,429 blocks and 69,124 explicit native bytes. Its
1,656 remaining I items do not establish full semantic feasibility. Replacing
the 313-item probe driver would make at most 1,969 items available
for all replacement semantic code; future control/diagnostic state is not yet
priced. Existing parser baseline is I = 6,460, W = 2,571, F = 87,
blocks = 1,485 and explicit bytes = 63,384. Three additional arrays model 774 owner-width
cells before the new State and output/control temporaries; this is not measured
successor admission. Existing I/W ceilings are 8,192 each and all other gates stay.
Passing just below the ceiling does not authorize broad implementation: report
per-function costs, genuinely removable probe-only code and credible remaining
headroom. Preserve failures rather than silently narrowing the semantic scope.
