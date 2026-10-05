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
