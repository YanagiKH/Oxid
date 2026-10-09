# Current Unit2 u8 accounting successor

This successor changes only the two named resource functions in an independently
materialized Unit2 package. The frozen archive and its enum-era adapter remain
byte-identical. It does not change production code, language expectations,
measurement history, or any resource ceiling. The same 21 resource-test names
remain mandatory; the semantic corpus remains 3,603 cases.

## Source-derived accounting

For `fn f()->(){}`, the unchanged validator and inventory cost 34 preflight units.
The unchanged mandatory construction expression contributes 168. Current
`u8_reservation::checked_bound` adds `2M + 3(O+I) + 4(R+E+I)`: with M=1, O=1
and other counts zero, that is 5. Therefore the mandatory reservation is 173,
and exact admission is 207. Tests reject 206 and the historical 202 before any
allocation, while preserving retained-before-scratch-before-work precedence.
The retained/scratch formulas and all default resource limits are unchanged.

For `mod c; use crate::c::X as Y;` with child
`pub struct X{} pub fn X()->(){}`, import commit still occurs at finish budget43.
The original replay, grouping, import transaction and freeze checks total46.
The source-order scan traverses two modules and four items, costing2+8=10.
The reservation scan costs two module visits, four item visits and two bindings.
The candidate names Y and X each mismatch u8 on their first byte, costing two
units each; total reservation scan=2+4+2+4=12. Finish is therefore exactly68.
Its conservative mandatory extra reservation would be24, not22: the bound
permits a second compared byte on each binding. These are deliberately distinct.

The current test executes every budget0..68, retaining transaction failure
origins and asserting complete paired alias/seen provenance after commit43.
For each budget46..68 it checks the exact hand-derived post-graph event prefix
(operation, origin and units); every failure has the expected next debit's
terminal diagnostic and no Frozen observation. Thus67 fails and68 succeeds.

## Reproduction

Run `python -B tests/qualification/unit2_u8_current/test_current.py` for adapter
pin, exact function-scope, inverse-restoration and mutation-denial checks.
Run the source-binding runner's current Unit2 lane for full independently
materialized debug/release resource and corpus qualification. Existing historical
runs do not qualify this current source.
