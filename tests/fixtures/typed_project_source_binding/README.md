# Explicit current and archived compiler input views

This adapter restores retained gates after public project syntax activation.
It does not create new language expectations or qualify current native execution.
All existing Unit1, Unit2, and Unit3 published packages and manifests stay unchanged.

`current-source.json` binds 120 actual inputs: the original 117 members plus
`src/frontend/parser/activation_tests.rs`, `tests/typed_frontend.rs`, and
`tests/typed_project_dispatch.rs`. The last two are integration-test inputs;
`typed_frontend.rs` existed before activation but was not an old manifest member.
Every member's bytes, length, and hash are checked, with exact membership under
`src` and `native`. The new package and all manifests live outside this inventory.
The adapter package manifest excludes itself from its exact member list.

`authority.json` pins the current manifest, the unchanged published authorities,
the original compatibility runner and bridge dependencies, and the one resource
probe seam. `source-transition.patch` is exactly the reviewed 28,881-byte nine-file
activation patch (SHA-256 `04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8`).
No full source corpus or expected-output corpus is duplicated in this package.

## Admission and archived Unit3

Run with unoptimized Python, an explicit repository, and a fresh external output:

```sh
python3 -B tests/fixtures/typed_project_source_binding/run.py preflight \
  --repo . --output /tmp/new-source-admission
python3 -B tests/fixtures/typed_project_source_binding/run.py prepare-archived \
  --repo . --output /tmp/new-archived-view
```

Both routes verify current inputs before any materialization or child process.
The inverse patch is applied in memory with exact line offsets and byte context.
It removes the two files added by activation, verifies the inverse integration
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
assertion and all other payload bytes stay unchanged. A distinct derived package
manifest binds that single resource change. The archived artifact-manifest is
preserved as historical provenance, not retargeted as a current pass receipt.
The byte-identical supported compatibility runner then selects the current
120-member manifest and invokes the unchanged Unit2 runner, observer, normalizer,
and comparator. Their original 3,603 semantic cases and 21 resource tests execute
per profile with two Cargo jobs, incremental compilation disabled, and offline
locked dependencies. `--prepare-only` only prepares inputs; it produces no pass.

The outer receipt binds a fresh invocation and plan to current source, adapter,
resource seam, final derived package, exact child result, executable hashes, and
retained build/test artifacts. Inputs are checked again before a child starts and
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
