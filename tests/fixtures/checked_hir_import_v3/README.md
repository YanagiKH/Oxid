# V3 checked-import regression captures

The six .ox/.bin pairs are fixed compile-time inputs for checked-import,
AST-comparison and producer-diagnostic tests. capture-receipt.json records their
exact source and observation hashes and the original producer executable hashes.
All six pairs are consumed by Rust regression tests; retain the binary frames
and source bytes together.

The producer_build value is the original local capture-run label. It is not a
path required to run these tests, a downloadable artifact, an accepted Git
checkpoint or a claim that the current producer generated those historical
captures. The original label and hashes are preserved without relabeling.
Frame membership and source/frame hash agreement can be checked directly using
capture-receipt.json. Tests must continue rejecting mutated framing, raw fields,
source association and diagnostics independently of that historical label.
