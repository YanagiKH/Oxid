# Source 3B1 typing observer replay

This package reconstructs a checked source tree, materializes 54 verified source cases, runs the test-only source/type observer, and compares every complete observation with independently frozen source expectations. The public input package contains 24 files and references existing Git objects; it does not contain or claim all 94 files of the historical authority.

The selected input parent is `2e84c9de9d9b02b62283fff1902cf1a840254ac4`, tree `86b90e5cb5eaf03a81f9faaa3b4f494248d2c60c`. `candidate.patch` applies the frozen core and passive test-only observer overlay. The resulting 1,297-file tree must match `full-source-closure.json` exactly before building or reusing a binary. The working checkout is never edited.

The pipeline invokes the published input verifier under optimized Python with bytecode writes disabled, materializes all 65 source bodies in original file order, and verifies the exact semantic/control/source projection from public input manifest `2cba1dbd200d75fbeb33b504b08279d89d2baa1784f88718413305446d458c35` to historical authority `7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039`. The unchanged external comparator receives those verified maps and source bytes through a narrow public authority adapter. No expectation, type, origin, ID or callback enters the compiler.

## Replay

Requirements are Python 3.12 or newer, Git with the selected input parent and its original base objects available locally, and the Rust/C/C++ build toolchain. The recorded local qualification used Rust 1.99.0 on x86_64 Linux. Each output path must be absent. Supply the independent review report that binds this exact package manifest.

```sh
python3 -B replay.py --repo /path/to/Oxid --out /new/debug-replay --profile debug --review /path/to/public-replay-review.md
python3 -B replay.py --repo /path/to/Oxid --out /new/release-replay --profile release --review /path/to/public-replay-review.md
```

Each invocation runs verify, materialize, request, observe and compare in one fresh staging directory. Builds use one Cargo job and disable incremental compilation. Requested sources, limits, source closure, binary, actual named test execution, exact 54-artifact roster and complete outputs are all bound in the receipts. An error is preserved in `result.json` with zero qualification credit; first outputs and logs are never overwritten. Output inside this package or the input Git repository is rejected before any directory is created, including paths that resolve through a symlink.

For an audited replay on the qualified host, `--reuse-binary /path/to/debug-test` (or `release-test`) accepts only the exact previously reviewed profile binary identified by this package's manifest. Reuse still reconstructs and verifies the complete source closure and reruns public input verification, materialization, requests, observations and unchanged comparison predicates. It does not claim that a new build was performed.

## Scope

The observer exports source/declaration/binding identity, append-order HIR IDs and roots, complete type slots including Groups, exact diagnostics, and bounded reservation/resource inventory. It does not count, lower, verify ownership or execute an Oxid source program. `WIRE-MAPPING.md` and `ACCOUNTING.md` describe the wire format and checked observation/auxiliary bounds. `compare.py` retains its historical entry point unchanged; this public driver uses its checked case, roster and execution predicates through the explicit public projection.

The current v4 comparison requires one HIR reservation per resolved ArrayLiteral, in actual preorder, including zero literals and type-error cases. Superseded v3 normal results are retained under `evidence/history`; current v4 normal comparisons and candidate-copy sensitivity are separately named.

Normal 54-case comparison and the historical protocol receipts are separate from the complete guard/failure/work control qualification. Evidence files are historical results, not replacement expected values or an oracle learned from compiler output. This replay does not activate either array gate or qualify the two deferred RHS snapshot effect cases, source association, ownership, lowering, runtime/fuel, native execution, Q, pilot activation, hosted CI, or untested hosts.
