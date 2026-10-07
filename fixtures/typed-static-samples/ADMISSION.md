# Static component admission

## Current separate AST1 consumer

The separate `ast_static_main.ox` consumer passes ordinary native admission
with I = 7,741, W = 3,907, 95 functions, 1,818 blocks and 79,048 explicit native
bytes including the eight-byte process wrapper. Headroom under the unchanged
8,192 I/W ceilings is 451/4,285. No input, scalar-slot, storage, fuel or other
admission limit was raised. It validates the complete AST1 boundary and clears
scratch storage before resolution and typing.

The combined source-to-type `typed_main.ox` candidate remains refused at
I = 8,778. Admitting a separate consumer does not qualify that combined root.
[README.md](README.md) separates completed local semantic, validator and I/O
checks from pending integrated current-source and hosted qualification. Those
local results are component evidence, not provider or self-hosting admission.

## Historical carrier and canonical observer

This is a carrier and canonical-observer precursor. It does not resolve or type
check candidate programs, and its successful suffix remains STF1 tag 2.

| Combined program | Native inventory I | Owner width W | Explicit native bytes, including wrapper |
| --- | ---: | ---: | ---: |
| Existing bounded parser | 6,460 | 2,571 | 63,384 |
| Initial static carrier | 7,404 | 3,645 | 75,020 |
| Exact keyword-table carrier | 6,536 | 3,925 | 69,124 |

The last program has 96 functions and 1,429 blocks. All ordinary native gates
pass, with the existing I/W limits of 8,192. There is no compiler admission or
fuel-rule change in this precursor.

The keyword recognizer preserves the same 35 exact spellings and token kinds.
Four 35-cell columns store three exact base-128 spelling chunks and length/kind
metadata. At most four ASCII bytes form each chunk, below 2^28. Length and all
three chunks must match. Literal and binding owners both remain charged: eight
owners total 280 cells. The keyword module falls from 1,122 to 254 inventory
items. Its classifier has 204 scalar slots within the existing per-function
limit. The table scan changes component work and fuel usage; ordinary bounded
controls pass without increasing fuel limits.

The original lexer wrapper also admits: I falls from 2,157 to 1,289, W rises from
3,276 to 3,556, and explicit native bytes fall from 31,984 to 26,088.

The precursor's focused qualification executed:

- 400 existing lexer observations across reference and native modes
- 139 parser cases in each mode: 118 canonical comparisons and 21 grammar-site
  refusals, plus 41 corruption controls and two transport cases per mode
- 16 complete static carrier inputs in both modes, matching the initial
  carrier's complete bytes, including signed MIN/MAX and reserved cells
- Nine canonical static-observer tests, including genuine public-function
  project routing, unknown-type diagnostic routing and exact source identity

The parser controller retains its exact historical source-membership check.
The precursor's four extra modules are not in the parser root's dependency
closure; parser replay uses a snapshot with its original exact roster and the
new keywords. The initial direct replay's membership rejection is retained.
The separate combined carrier replay covers all four additional modules.

The 1,656 remaining I items are not proof that the intended complete resolver
and checker fit. Replacing the 313-item probe driver gives only an arithmetic
upper bound of 1,969 items for replacement semantics and state. That driver
contains synthetic stress work and actual source observations; useful work must
be retained. Complete semantic control/diagnostic state is still unpriced.
Those measurements did not admit broad semantic implementation or provider
activation. The subsequent separate consumer is measured above; provider
activation still requires a separate gate.
