# Bounded native portability review

The six approved native portability invocations pass independent review: scalar
success and Unicode/CRLF owned runtime overflow in each profile, plus debug
unused-recursion admission denial and regular occupied-output rejection. Four
rows produced and actually executed source-free ELFs. These are four additional
bounded portability executions, separately attributed from the original 96
primary and three smoke ELFs.

The exact component manifest is
`695e67a3074131d4da30af4cc5c1c17fe893ee7c6d123cedeed777db2427030b`,
wrapper `fcc7f17dd26de7ee20078db8fd6215bc9a62594eb9a5d731bb1cb8676a907501`,
and six-row roster
`d0480056fb539fa95b59afdb3467bb3ad576bc90e1950d0f3ab2e9832a9e412b`.
The frozen native controller/adapter/overlay remain byte-identical. The wrapper
translates explicit workspace/tool locations and verified source-only fixture
roots, retaining original checking, source hiding/restoration, tool execution,
ELF execution and no-clobber behavior.

The initial wrapper's unconditional version probes would have run before
admission. They were moved to build/setup. The final invocation verifies a
successful build receipt, exact compiler/LLVM executable bytes, explicit
environment paths and the resolved LLVM library inventory without starting an
external process. Library contents, symlink targets, directory membership and
path selection are bound with streaming hash and traversal limits. Four
independent final-wrapper synthetic controls confirm original invocation order
and reject changed library bytes or directory before worker execution. The
package's earlier controls retain their preceding wrapper identity; this review
uses the independently rerun exact final-wrapper controls.

The build wrapper is
`a3b731fa1f962fdc64b791816dbed614ca4fd4157d20ccb3c1adccbe1f864e46`;
its qualified build/tool set is
`860eb9cac65985afa890bbeae25e104d0366ad5c0c8031e9a1a584f721f3f396`.
Both profiles and all six inner/outer receipts join to those identities. All
original source paths were absent at the prescribed checking boundary and during
ELF execution, then restored with exact source bytes. The ELFs ran in fresh
directories containing only the copied program with the recorded minimal
environment. Both rejection rows used zero external tools; the occupied output
and sentinel bytes were retained. No extra receipt is admitted to this roster.

The pre-execution expected path projection is
`13962c9b7b248b338fc884e9745bee14e31b6ace8be0a8ff79f5b816c98d7910`.
It copies frozen semantic stream/status expectations and substitutes only the
prescribed fixture prefix and output literal. The projection was independently
reproduced before execution. Its exact request selection reflects the unchanged
controller's native-default versus no-clobber branches. The initial source-only
projection mistakes remain preserved and were fixed before any invocation.

All six actual compiler/driver streams and four ELF streams/statuses match that
projection. The final semantic comparison independently reproduces byte-for-byte
at `92482998c9ec7f996208a8739e5c4dc661dbb11d45983d1cf1c9c864b6dcd154`.
All four emitted LLVM texts also match their originally qualified same-profile
counterparts after only decoded diagnostic-path and checked length relocation;
all other LLVM bytes are equal. Normalized text was never executed.

Evidence: `native-portable-apparatus-v3-review.json`,
`native-portable-translation-review.json`, `native-portable-results-review.json`,
`native-portable-semantic-recomparison.json` and `native-portable-controls-v3/`.
This establishes the bounded portability shell for the supported Linux x86_64,
Rust 1.99.0, LLVM 19.1.7/O0 scope. It adds no native target, portable ABI or full
corpus replay claim. Final publication assembly/bridge wiring is separate.
