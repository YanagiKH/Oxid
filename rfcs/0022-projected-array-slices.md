# RFC 0022: call-only projected fixed-array slices

Status: bounded implementation contract; not a qualification or v1.0 claim.
Base: local `56f7dec0ce499512b45b2bae2c67e834f900c530`, tree
`f052fc27b068042ef7c5835e4be001e5a5e49123` (published PR35 equivalent).

## Observable boundary

Direct call arguments may borrow a named-root record field path ending in an
existing fixed bool/i32/unit array as a scalar slice parameter only:
`sum(&batch.samples)` and `bump(&mut batch.samples)`. Nested record paths are
bounded by the existing 64-hop limit. Length zero is admitted. Every field hop
retains nominal identity and visibility checks. No array copy is made.

Explicit whole-record-reference projection uses `&*p.samples` or
`&mut *p.samples`: in this restricted borrow-argument grammar, the star marks
explicit reborrowing of the named reference parameter, and the following path
selects the array field of its whole record. It is not a general dereference
expression. Ordinary `&*p` slice forwarding is unchanged. A projection supplies
only `&[T]` or `&mut [T]`, never an exact fixed-array parameter. Modes and scalar
element types must match exactly; shared authority cannot be upgraded.

All loans retain the complete outer owner as permission identity. Different
fields of the same record are not disjoint: shared/shared may coexist, while
exclusive/shared and exclusive/exclusive conflict. Loans begin in left-to-right
argument order, remain live through the call, and release normally on return.
Explicit reborrows suspend incompatible parent access and restore it afterward.

Pilot: Batch contains independent metadata and `[1,2,3]`; generic bump changes
only samples to `[2,3,4]`; generic sum returns 9 and metadata is unchanged.

## Representation and validation

Loan descriptors retain whole-root `AccessBase` plus a nominal field-ID path,
empty for existing whole borrows. Raw verification independently resolves every
hop using checked declarations, requires a final fixed scalar array, validates
its scalar slice formal, and retains root availability and conflict checks.
No producer-provided physical offset grants authority.

Runtime references and loans carry a checked view independently of root and
permission identities: root-relative byte offset and exact aggregate descriptor.
Whole views have offset zero and the root aggregate; projected views identify
only the complete selected array. Handles must agree with their active loan's
view as well as root/generation/permission. Projection extents are independently
checked against the owner's padded storage bounds before use. Index bounds use
the selected array length, so empty sentinels and adjacent fields/padding never
become elements. Native lowering stages the verified field address and length
when the borrow argument is evaluated. Slice reborrows preserve that view.

## Ordering, fuel and resources

Existing expression and call fuel remains unchanged: projection borrowing is a
view, not element copying. The existing PrepareBorrow charge occurs before its
state transition. Index reads/writes/len keep their one-operation charge and
store order remains complete RHS, index, access fuel, bounds, final store.
Existing aggregate initialization/move/teardown width charges remain unchanged.

New path payloads contribute to source inventory, raw metadata and verifier
work. Enclosing AST/HIR/loan and runtime-handle layouts must be measured, with
runtime reference and loan growth charged by actual retained bytes and 64-bit
cell counts. Native emission/path walks must be charged. All current caps remain
unchanged; additional storage can therefore cause earlier resource rejection.
Frozen predecessor ledgers and qualification packages remain immutable and do
not qualify this source identity.

## Exclusions and evidence

No aggregate field extraction/replacement, partial moves, scalar/record field
borrows, element borrows, ranges, stored/returned references, heap collections,
implicit forwarding, grouped/temporary roots, broader native platforms or
raised ceilings. Legacy/OXBC remain unchanged.

Acceptance requires focused failing/pass controls, nested/privacy/module cases,
bool/unit/empty views, whole-root aliases, parent restoration, moved/immutable
rejections, forged raw paths/types/views/generations, padding/sentinel isolation,
fuel-before-store, resource measurements and reference/native source-free parity.
Ordinary regressions, independent review, current-source qualification and
exact-head hosted CI remain separate gates before merge.
