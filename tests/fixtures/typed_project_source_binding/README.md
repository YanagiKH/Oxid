# Explicit current and archived compiler input views

This adapter restores retained gates after public project syntax activation and
private fixed-array implementation and its tests.
It does not create new language expectations or qualify current native execution.
All existing Unit1, Unit2, and Unit3 published packages and manifests stay unchanged.

`current-source.json` binds 185 actual inputs: all 133 tracked `src`/`native`
blobs, the ten unchanged historical non-source paths, and exactly 42 published
compile-time fixture inputs. Including `Cargo.toml`,
`Cargo.lock` and `build.rs`, this is 136 compiler bodies. Its immutable published
source checkpoint is `a5fb98b4f1ad2fa95ee6e4637f4e9d7700cbe909`, full tree
`b30b0628c45e4a308bbb0ae5b35122794cd7ac12`.

`combined-authority.json` pins a 376,315-byte `combined-transition.patch`
(SHA-256 `8ef58e282f1e37a7c04e653222fb74cb40f144aaa4364723773a608df2182683`),
the ordered 80 changed paths: 38 compiler-source paths and 42 exact compile-time
fixture additions. Its 52 additions comprise ten source files and those 42 fixtures. The patch is the exact
`git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE CHECKPOINT -- PATHS`
from published `595f681c2a906d686ddea90c65d060cff97e0a75` to that checkpoint.
The original 133-member current manifest is preserved byte-for-byte as
`formatter-source.json`. Admission verifies actual current bytes and exact
membership, reverses the combined patch (including all 42 fixture additions),
and verifies every formatter-view byte.
Only then does it reverse the unchanged 75,550-byte, eight-path formatter patch
and verify the 129-member `predecessor-source.json`; the unchanged historical
transition finally reconstructs the 117-member selected archive.

The fixture inventory is derived only from the exact published
`src/frontend/oir/owned/source/array_types_tests.rs` bytes (44,176 bytes,
SHA-256 `10687b76ac4c048d21467653b55a4c322ac011209f6d1ebd503ce2fc778810cc`).
Its 47 literal `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), ...))` references
resolve to the separately pinned 42-path roster. Admission checks that source
identity, exact macro syntax, reference count/order and unique roster before
reconstruction. This is no generic asset allowance. The ten historical retained
non-source inputs remain separate and byte-identical. The one empty `guard-empty`
fixture is reversed only through its exact Git empty-blob addition header and
verified empty current bytes; nonempty or malformed empty additions reject.

All three stages have literal hash, length and ordered path-scope pins. The
current manifest, preserved formatter manifest and combined authority also have
literal hash/length pins before reconstruction. The formatter authority,
predecessor authority, predecessor manifest and both older patches are unchanged.
Each stage verifies exact reconstructed membership and every member's length and
hash. None executes a compiler. The package lives outside the compiler inventory;
its package manifest excludes itself. These identities do not establish a new
semantic, native, formatter or array-language qualification.

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
The current, formatter and predecessor manifest pins, plus the combined and
formatter authority pins, precede reconstruction; ordinary modified package
members still fail their package identity first. Transition metadata, the old
prefix, and the appended delta are separately checked before reconstruction.
Every inverse patch is applied in memory with exact line offsets and byte context.
The final historical stage removes the two files added by activation and nine array files,
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
first verifies the original resource probe to `project_recovery:false` derivation
against the unchanged predecessor authority. It then adds exactly
`arrays:ArraySyntaxPolicy::Closed` to that isolated Parser initializer. The new
2,007-byte resource SHA-256 is
`7c3b0d1cc06124be9a432525acdad2bf622f1061c6f474fc8fa049ce267e360f`, separately
pinned in the combined authority. Reversing the full current initializer seam
restores the historical resource byte-for-byte. Every resource assertion stays
unchanged. The versioned `unit2-record-aggregate-observer-v1`
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
185-member combined-source manifest and invokes the unchanged Unit2 runner, current-only observer adapter, unchanged normalizer,
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
