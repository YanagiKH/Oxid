# Current bounded stdout qualification

`scripts/verify_bounded_stdout_native.py` defaults to both ordinary Cargo
profiles. It never builds Rust. The enum gate's existing source identity,
Cargo JSON artifact streams, command receipts, profile flags and binary hashes
are admitted using the enum/stdin helpers before any selected program runs.
The complete 262-member current authority and 207 compiler/build bodies, exact
checkout head/tree/event, tools, helper inputs, build receipts and binaries
are checked before and after execution. `--profiles debug` is an explicitly
partial local rehearsal, never a claim that release passed.

The frozen roster selects 71 exact tests (including two ignored reference
parents and two inert ordinary children). Every module listing and exact
selection refuses zero, missing, extra or duplicate tests. The parent proofs
remain reference-only. The controller separately emits and executes native
ELFs for each raw/parsed-source row, with empty native working directories and
cleared native environments. Selected reference children's harness streams
are retained separately from actual program stdout; direct exits cannot be
accepted as ordinary libtest success.

Per profile, the fixed effect roster has 10 raw, 28 fuel, 5 actual-pipe,
8 injected-return and 11 parsed-source pairs. Public checks perform 40 CLI
invocations, 8 separate native executions, and 3 groups of actual/injected
terminal controls. The public output-then-range-error row preserves ABC and
requires exact reference/native stderr and status 1. CLI setup calibration
forwards Rust startup before failing ordinal 2; native setup fails ordinal 1.
The FIFO control proves that failure precedes source loading. All failures
retain bytes, commands, identities and terminal status, including timeouts.

The independently authored `fuel-oracle.md` is copied verbatim from the
pre-execution ledger, including the append-only projected 179 correction.
The ten source `.txt` bodies are literal copies of the reviewed
`source/output_source_tests.rs` constants and their three explicit
invalid/empty substitutions. They are materialized as `.ox` only in fresh
evidence directories, outside the repository's application registry. The
module alias uses the reviewed `crate::child::relay` spelling. No expected
status or byte sequence is derived from compiler output.

`faults.c` and `calibration.c` preserve the bounded private process proof's
identity-selected syscall-return controls. `setup_ordinal.c` preserves the
public proof's exact-executable, exact-descriptor and calibrated-call control.
They are compiled afresh with the admitted clang, calibrated, and identity
checked. These are injected outcomes, separately labeled from real closed
and one-free-byte nonblocking pipes; they do not claim actual signal delivery.

The separately owned artifact controller is dispatched once with each
admitted CLI via its existing `--oxid`, `--output`, `--native` interface.
Its source copies, framing oracle, producer publication rules, standalone
loader, 81-cell witness and malformed-input matrix remain independent. Its
report is linked to the same before/after CLI identity. Existing enum eight
groups and stdin 77 cases remain separate unchanged gates.

Focused refusal controls (no Rust or LLVM build):

    python3 -B tests/qualification/bounded_stdout_current/test_controller.py -v

Full native gate, after the enum gate has produced both ordinary profiles:

    python3 -B scripts/verify_bounded_stdout_native.py \
      --repo "$PWD" --output "$OUT/stdout" --build-evidence "$OUT/enum" \
      --llvm-bin "$OXID_LLVM_BIN" --expected-head "$Q" --event-sha "$EVENT_SHA" \
      --source-manifest-sha256 3ae8ee6cbaf6697f0735fcf4e0cb345724d76c2bae6f5046fdfb441d02ecc936

CI always uploads the new stdout evidence directory, including nested public
artifact evidence, even when qualification fails.
