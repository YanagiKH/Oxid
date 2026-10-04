# Explicit current and archived compiler input views

This adapter restores retained gates after public project syntax activation and
private fixed-array implementation and its tests.
It does not create new language expectations or qualify current native execution.
All existing Unit1, Unit2, and Unit3 published packages and manifests stay unchanged.

`current-source.json` binds 133 actual inputs: all 123 tracked `src`/`native`
blobs plus the ten explicit retained non-source paths. The formatter successor
adds four source/test files and changes four existing source files. Its source
checkpoint is `8a08a2908b2ceb73c80112e6ddd82e2dbda91976`, full tree
`afa181dab5aa3341ceae4f7b882a81a635e08fa0`.

`formatter-authority.json` and a pinned 75,550-byte `formatter-transition.patch`
record only that eight-file change from published main
`17ed3de243785006ab888d87af4428d06117b21d`. Admission verifies current bytes and
exact membership, reverses the formatter patch, then checks every reconstructed
predecessor byte against `predecessor-source.json`. That file preserves the old
current manifest byte-for-byte. The predecessor has 129 inputs: 119 source/native
files plus ten retained paths, or 122 compiler bodies including Cargo.toml,
Cargo.lock and build.rs. Only after this check does the unchanged historical
transition below reconstruct the 117-member archive. Both transition stages
have literal hash, length and path-scope pins; neither executes a compiler. The 12 paths absent from
the unchanged 117-member selected archive are the activation and integration-test
inputs plus nine array implementation/test members. Every member's bytes, length,
and hash are checked, with exact membership under `src` and `native`. The new
package and manifests live outside this inventory; its manifest excludes itself.

The unchanged predecessor `authority.json` pins its manifest, published authorities,
original compatibility runner and bridge dependencies, and one resource probe
seam. `source-transition.patch` is a composed 605,300-byte, 56-path transition
(SHA-256 `63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18`).
Its first 28,881 bytes preserve the reviewed nine-file activation patch
(SHA-256 `04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8`).
The remaining 576,419 bytes are one cumulative 47-path source delta from
`0ef3be1df3643febdff1f859a4eb1ce567ab8164` to the source checkpoint
(SHA-256 `d76d1aec1a4a15912c47fd1b80e6800006c49fcbaa5b96b9817b4479db2b0d62`).
The two path sets are disjoint; every patch path occurs once. The authority
records both components, ordered scope and exact Git diff recipe. Their inverse
reconstructs the unchanged published archive before any files are written.

The predecessor manifest's `base_head` retains historical activation provenance.
Its `reviewed_source_head` is the immutable source checkpoint
`f8a30b2443d9d7a89498e0c243f75072cbf26c2f`; `source_only_tree` is its exact
full tree `18a4611f46b3cdf59f37a9f2241dc4e39e2d8ba6`, before binding integration.
These fields do not claim that the checkpoint or preparation has passed CI.
All public/current-source array gates remain closed. No semantic expectations
or full source corpus is duplicated in this package.

## Admission and archived Unit3

Run with unoptimized Python, an explicit repository, and a fresh external output:

```sh
python3 -B tests/fixtures/typed_project_source_binding/run.py preflight \
  --repo . --output /tmp/new-source-admission
python3 -B tests/fixtures/typed_project_source_binding/run.py prepare-archived \
  --repo . --output /tmp/new-archived-view
```

Both routes verify current inputs before any materialization or child process.
After exact adapter package admission, the trusted helper's literal current-manifest
pin rejects
coherently rewritten current/package metadata before archive reconstruction.
The current and predecessor manifest pins, plus the formatter authority pin,
precede reconstruction; ordinary modified package
members still fail their package identity first. Transition metadata, the old
prefix, and the appended delta are separately checked before reconstruction.
The inverse patch is applied in memory with exact line offsets and byte context.
It removes the two files added by activation and nine array files,
restores the changed array source files, verifies the inverse integration
test's independently pinned old identity, and omits that test from the archived
qualification view. All 117 original selected-current members must then match
their unchanged published hashes before a single reconstructed file is written.
Preparation records zero compiler executions and cannot produce a semantic pass.

The unchanged Unit3 platform bridge receives `/tmp/new-archived-view/archived-selected`
as `--repo`. It still admits only its original selected-current, core-v1,
platform patch, and old test authorities. Existing source/native/mutation
qualification therefore describes explicitly reconstructed archived core-v1.
It is not evidence that the activated current compiler matches archived outputs.
CI preserves the source-binding receipt beside all original bridge/build/plan/
collection/comparison receipts. Original 304/300/220 plans and all comparators
remain unchanged.

## Current Unit2 and retained Unit1

```sh
python3 -B tests/fixtures/typed_project_source_binding/run.py run-unit2 \
  --repo . --output /tmp/new-current-unit2 --cargo /path/to/cargo --rustc /path/to/rustc
```

The adapter copies the verified historical Unit2 package into fresh output and
adds exactly `project_recovery:false` to its isolated Parser initializer. Every
resource assertion stays unchanged. The versioned `unit2-record-aggregate-observer-v1`
adapter also changes exactly four pinned seams in the isolated `semantic/observer.rs`:
the AggregateTy import, a checked record-ordinal projection helper with four adapter
controls, the Owned projection, and the Reference aggregate-field projection.
The original observer SHA-256 `f2403aace53b6255a94b8b3ec0290db025638c5af94571d5729355672681fb00`
and derived SHA-256 `ddeff8bd0acfaa9af0301c74f3aabd26ef69954eb5a216fdd9cc21e1837901a3`
are independently pinned. All scalar/record JSON stays unchanged; FixedArray
projection panics because this qualification does not open array source gates.
Every substitution must occur once, and reversing them restores the historical
observer byte-for-byte. The package changed-member allowlist is exactly the
resource probe and current observer, plus their derived package manifest.
The original historical semantic package manifest remains historical evidence;
it is not retargeted as a current observer authority. The archived artifact-manifest is
preserved as historical provenance, not retargeted as a current pass receipt.
The byte-identical supported compatibility runner then selects the current
133-member formatter manifest and invokes the unchanged Unit2 runner, current-only observer adapter, unchanged normalizer,
and comparator. Their original 3,603 semantic cases and 21 resource tests execute
per profile with two Cargo jobs, incremental compilation disabled, and offline
locked dependencies. `--prepare-only` only prepares inputs; it produces no pass.

The outer receipt binds a fresh invocation and plan to current source, adapter,
resource seam, final derived package, exact child result, executable hashes, and
retained build/test artifacts. The four `current_unit2_aggregate_adapter` Rust
controls run automatically on each freshly verified profile test binary after
the unchanged corpus/resource qualification. Exact listing and results, binary
and assembly identities, commands, stream hashes and profile receipts are retained;
any missing, extra, ignored or failed test rejects the current pass. Bounded Python
controls inspect admitted bytes/protocol only and do not claim Rust execution. Inputs are checked again before a child starts and
after it returns. Failed attempts retain command output and a failure receipt;
occupied outputs are rejected without altering prior evidence. A fresh gate run
is required after any admitted identity changes.

Unit1's existing `--original-only` interface retains all 31 original public cases
and 496 invocations. Four deliberate syntax migrations require separately
reviewed replacement public contracts. This adapter does not change or supply
their expected results.

## CI order and validation limits

1. Run source-binding negative controls, then admit actual current inputs
2. Run current Unit2 through the unchanged compatibility machinery
3. Run ordinary current-source Rust, formatting, Clippy, private seals, and Unit1
4. Reconstruct and verify archived selected inputs before existing Unit3 gates
5. Retain both current and archived labels, fresh results, and failure records

Run `python3 -B -m unittest discover -s scripts -p test_typed_project_source_binding.py -v`
for bounded adapter controls. These are admission/protocol tests, not compiler
qualification. No regenerated manifest or preparation receipt replaces an actual
semantic/native/mutation run. Independent implementation review and fresh gate
execution are required before integration claims.
