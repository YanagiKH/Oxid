# Independent review: portable parser provisional v1

Verdict: **FAIL for source/recipe admission as written; do not activate it.** The two provenance/environment defects below require correction, and the documented build/collection host-consistency requirement is not enforced. This verdict is limited to the provisional portable core. It is not a compiler, parser semantics, amended-contract, complete collection, or hosted-CI verdict.

## Exact reviewed target

- Producer directory: `/workspace/shared/oxid-reset-recovery-20261003/portable-parser-v1`
- Checkpoint SHA256: `37ffb1cf0c9b4be7f324079c39d472fb81f09cc0473219f582939befb0b8d659`
- `portable.py` SHA256: `879390cfc59eef3fbff432bba04b79f245bccb3e80e3139321f13cae22dec59c`
- Authority SHA256: `8c2699170926e4bf4bb1b004b9e30980b9f5e66fa719f450f5474e40a76257db`
- All 31 checkpoint members were independently verified before execution. Review tests import the unchanged `snapshot/` copy. No producer source, compiler source, expectations, or historical receipts were edited.
- This review read the applicable software-engineering and code-verification skills and their verification matrix. Actions were local-only and bounded. No network, third-party writes, Cargo/Rust build, C compilation, or candidate corpus execution occurred. The synthetic integrated fixture does call the adapter's ordinary native-tool version and `-print-prog-name` identity probes; zero-compiler counters in the synthetic reports mean zero compilation/build executions, not zero identity-probe subprocesses.

## Findings

### 1. High, high confidence: ambient Git overrides falsify recorded checkout HEAD/tree

Location: `portable.py:167-168`, consumed by `verify_checkout` at 176-193, `prepare` at 251-255, and `session_at` at 314.

`git()` inherits the complete caller environment and resolves `git` using its ambient PATH. `git -C <directory>` does not override `GIT_DIR`. A real read-only control sets `GIT_DIR` to the historical worktree's Git directory and calls `verify_checkout` on the actual current checkout. It succeeds, recording historical HEAD `d9e6b9bf172abd5e15da7212c9e6224e29ccc768` and tree `305a1226d6ca1abd3bdea765200da082b9d9bc75` for the current checkout whose real HEAD is `02281a24a2bfa029720c7be243e65c0ed2d518e4` and tree `f0180761c9cf44ab7a74f2a381f4238f14c32ee2`. The reciprocal directory override is also admitted by `verify_checkout`.

The complete compiler file bytes remain pinned. The defect is false HEAD/tree provenance, which makes the receipt unsuitable for exact-PR-head qualification. It does not demonstrate changed compiler input admission.

Repair direction: use an explicit trusted Git executable with a minimal environment that excludes all ambient repository, worktree, index, object, replacement/configuration overrides. Verify the historical and current heads/trees in their intended directories. Retain poisoned-environment controls for both directions; do not merely unset `GIT_DIR`.

Evidence: `heldout_controls.py`, `heldout-report.json`, `heldout.stdout`. Both directory controls fail their independent clean-environment oracle.

### 2. High, high confidence: unselected toolchain executables remain eligible for execution

Location: `portable.py:196-206` and `334-349`; unchanged `frozen/helpers/build.py` resolves `which`, `rustc`, and `cargo` by PATH.

`verify_toolchain` closes only the selected 62 files. `minimal_env` then prepends the entire toolchain `bin` directory to PATH. A private review view hard-links the exact 62 official files without changing any bytes, adds an ordinary harmless `bin/which` script, and passes `verify_toolchain`. The same command lookup used by the immutable build helper executes the unpinned script. A separate harmless `bin/as` shadow also passes selected-file verification and executes through the build environment's PATH.

This shows an executable input outside the admitted compiler selection can influence the command recipe. The native-subtool control exercises PATH lookup only; it does not claim an actual C compiler invocation. The allowed unclosed host dynamic dependencies are a different boundary and do not close these newly admitted toolchain executables.

Repair direction: expose only the exact selected Rust executables through a fresh private executable view, with the necessary links/targets verified, while resolving host-trusted tools explicitly; alternatively close every executable-search directory's permitted members. Include both `which` and a native-subtool shadow in rejection controls. Preserve the intended Rust sysroot identity when moving executable discovery into a private view.

Evidence: `heldout_controls.py`, `heldout-report.json`. `which rustc` returns `UNPINNED_WHICH_EXECUTED`; `as --version` returns `UNPINNED_AS_EXECUTED`. No Rust or C compiler was run.

### 3. Medium, high confidence: declared build/collection host consistency is not checked

Location: `portable.py:435-438`, `480-490`; DESIGN states that the adapter measures build/collection host and requires consistency.

`verify_build` checks only whether the build host is a supported tuple. `admit_execution` checks the collection host against the collection invocation, but never compares it with the build invocation or session host. In an integrated admission control, a real source session plus an explicitly synthetic one-case ledger-empty build/collection chain passes the derived comparator's `execution_manifest` while the build uname release/version are `SYNTHETIC-DIFFERENT-BUILD-KERNEL` / `SYNTHETIC-DIFFERENT-BUILD-HOST` and collection uname is the actual host snapshot.

This is a same-session source/receipt provenance conformance defect, not a fabricated full 638-row success. It does not imply that different Linux kernels are inherently semantically incompatible. The full frozen contract correctly rejects the one-case fixture. Actual same-runner provenance cannot be cryptographically established from these receipts, but consistency of the measured snapshots can and should be checked to support the documented requirement.

Repair direction: define exactly which measured host fields must agree across preparation/build/collection and enforce that equality, including the recorded Python identity where intended. If only the normalized ABI tuple is deliberately required, narrow the specification explicitly and independently review that decision.

Evidence: `session_controls.py`, `session-report.json`. The synthetic fixture is retained under `synthetic-session/` and is clearly labeled as synthetic in driver evidence.

## Requirements and evidence

| Requirement / risk | Fresh evidence | Result |
| --- | --- | --- |
| Immutable authority, exact historical 283 and derived 286 rosters, ten helpers | All checkpoint hashes; original 9 tests including two independent real source preparations and coherent source substitutions | Pass within tested core |
| Current compiler bytes versus current Git HEAD/tree | Real current/historical directory controls with poisoned Git environment | Fail: bytes pinned, recorded Git identity can be wrong |
| Selected 62-file Rust identity and executable recipe | Exact official selected-file view plus extra `which` and `as` controls | Fail: selected identity passes while unselected executables run |
| Cargo package/target/body/profile/freshness admission | Existing two-root synthetic debug/release Cargo controls | Pass for synthetic receipt checks; no real build evidence |
| Environment/config exclusion | Existing Cargo-config/environment tests; Git and PATH held-outs | Partial: covered Cargo/flag variables reject; two escape boundaries fail |
| Comparator transformation preserves predicates and raw checks | Pinned-source hashes, original derivation proof, independent AST comparison of all 36 other functions and all 16 retained admission-suffix statements | Pass: no lossy suffix rewrite found |
| Frozen raw/source/token/AST/predicate/relational checks | Fresh 36 normal and 36 optimized tests | Pass for preserved v5 regression surface only |
| Bound fresh session/build/collection paths, roster, nonce, complete evidence | Real preparation plus one explicitly synthetic integrated case; full-contract one-case rejection, shared-nonce rejection, extra-file rejection | Partial: useful integrated controls pass, complete negative matrix still absent |
| Measured build/collection host consistency | Integrated mismatched-uname synthetic admission control | Fail against DESIGN's stronger statement |
| Resource bounds | Source inspection: jobs=2, incremental disabled, 120-second per-case run timeout, raw-file post-run limit, observer event/byte budgets | Partial; see below |
| Fresh debug/release 638 rows and exact-head hosted gate | Not run or available in this review | Unverified; mandatory before activation |

## Resource and portability limits

The draft does bound build jobs and individual observer-case timeout. It does not bound the outer helper invocation, Cargo build, test-roster `--list`, native tool version probes, or Git reads by time. Stdout/stderr and the copied offline Cargo index have no explicit size cap; the raw output limit is checked after the candidate process returns. `identity()` and JSON reads load whole files. Before CI activation, an integrated runner must provide explicit job/process deadlines and practical evidence/disk bounds, and test failure preservation. These are uncompleted operational controls, not evidence of an observed parser hang or overflow.

Supported portability is deliberately Linux/x86_64/64 with exact Rust 1.99.0 selected bytes. A changed official Rust selection, different target, or incompatible system C toolchain needs new authority/validation. Python/native tool identity is measured rather than preapproved. Host dynamic libraries, system execution, and the kernel remain trusted inputs; this review does not require a new hermetic toolchain claim or remote attestation.

The dependency copy includes the pinned 64 archives/source files, and index metadata is not source authority. Source/compiler bytes remain pinned before and after source preparation. The permitted index is still an unbounded copied tree; hosted resource controls should account for it.

## Executed commands and outcomes

1. Snapshot/checkpoint identity validation: 31 members, exit 0.
2. `UNIT4_PORTABLE_HISTORICAL_REPO=... UNIT4_PORTABLE_CHECKOUT=... UNIT4_PORTABLE_CONTRACT=... python3 -B test_portable.py`, from `snapshot/`: 9 tests, exit 0, 8.233 seconds. This includes two real source materializations and synthetic Cargo controls.
3. Frozen comparator suite with verified contract override, `python3 -B`: 36 tests, exit 0, 3.781 seconds.
4. The same suite with `python3 -O -B`: 36 tests, exit 0, 3.783 seconds.
5. `python3 -B heldout_controls.py`: 6 controls, 2 AST-preservation passes and 4 expected-rejection failures covering 2 Git directory overrides and 2 unselected executable shadows; exit 1 reports the findings.
6. `python3 -B session_controls.py`: 4 controls, 3 negative-control passes and 1 host-consistency failure; exit 1 reports the finding. One additional real source preparation, no compiler/candidate execution.

The first held-out harness attempt could not hard-link across the separate shared/scratch filesystems. That is a test-fixture environment failure, retained as `heldout-first-attempt.stderr`, not a product finding. The isolated executable view was relocated to the scratch filesystem and rechecked; no selected official file was edited. Logs and report artifacts are bound in `review-checkpoint.json`.

## Required successor rebindings and remaining gates

Do not overwrite this v1 checkpoint. A new version must bind the repaired adapter bytes and authority, corrected executable-discovery recipe and negative controls, independently reviewed comparator successor, and the additive coordinate amendment's effective contract and freeze identities.

The comparator SHA constant, authority package-file identities, derivation proof/retained-region hashes, comparator-control identities, contract loader pins, collection contract/freeze bindings, and downstream observation/comparison effective-contract identity must agree on the successor. Frozen helper `run.py` currently has literal original contract/freeze pins and emits them in raw-normalized bindings; an effective-contract integration must explicitly handle those identities through a reviewed successor/rebind, without relabeling historical receipts or silently loosening the immutable helper checks. Changes to helper bytes require the corresponding ten-file map, helper manifest and derived overlay identities to be rebound. Preserve source/instrumentation invariants explicitly.

The two known v5 comparator ordering overreaches and original coordinate projection defect were kept separate and were not fixed or used as test answer oracles here. The successor must retain the original raw/source/semantic obligations in addition to its independent correction/amendment evidence.

Before activation, complete the integrated portable session/collection controls for malformed/stale/session-reused receipts, missing/extra/duplicate/zero executions, all profile bindings, cross-profile reuse, actual ABI mismatches, and the full expected file/row roster. Then run fresh real debug and release builds/collection and the full effective 638-row comparison, followed by the exact-PR-head hosted gate. No such qualification is established by this review.
