# Projected array slices: local validation boundary

Implementation contract: [RFC 0022](../../rfcs/0022-projected-array-slices.md).
Source base: local56f7dec, tree f052fc27 (published PR35 equivalent).

This ledger will record fresh integrated checks of the new source. Test
registration does not establish execution. Independent review, source-binding
qualification successors and exact-head hosted CI remain separate gates.
Predecessor manifests and archived observer inputs are unchanged.

Measured retained runtime representation on this 64-bit host: BorrowView16,
ReferenceHandle80 (10 expanded cells), LoanRuntime112 (14 expanded cells),
LoanDecl96 bytes. All actual retained-byte charges and cell counts use unchanged
admission ceilings. Views retain exact arrays and checked root-relative offsets,
while authority remains the complete root owner.

The first regression run exposed an accidental coupling between physical cell
counts and activation fuel. The successor separates stable logical activation
fuel from physical admission; unchanged-source fuel schedules are retained,
while resource-boundary fixtures independently account for the larger metadata.
Native arenas retain pointer/length storage and existing byte totals.

Measured source carriers: AST BorrowPlace56/Argument88, HIR BorrowPlace16/
Argument104, resolved program512, TypedBody144 (24-byte growth), typed function40,
sparse borrow entry72, FieldId16. Sparse capacity and paths share the existing
64 MiB cumulative source payload cap; no old source inventory walk was added.

## Fresh integrated checks

Debug all-target/all-feature suite passes. Focused new ordinary controls cover
forged nominal paths, exact-type rejection, same-root conflicts, permissions,
privacy at every hop, format round trips, modules, signed/empty bounds, and
coherent handle+loan view corruption across direct and child slice loans.
Fuel denial precedes bad-view access and the complete backing payload remains
unchanged. A successful paid store is checked byte-for-byte against neighboring
metadata, a poisoned empty-array sentinel and poisoned alignment padding.

LLVM 19.1.7 O0 debug projected gates pass: actual staged addresses/lengths,
64-hop paths, parent restoration, source-free success, every-budget mutation,
signed bounds and guard dominance. Clippy and actionlint pass. Release/native
public gates and Python/source-binding checks are separately reported with
retained command logs; absence of a current-source qualification successor
remains a merge blocker, not an inherited green claim.
