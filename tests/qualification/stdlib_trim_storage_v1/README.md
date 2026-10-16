# ASCII trim: fixed storage and mechanical predecessor

Baseline: `8debaa96c0238e3376567fa2c76517fda9937037`, tree
`daa438a1c817793c528123153bbc320c7fb2aae1`.

Live `stdlib/strings.ox` strips only boundary U+0009..U+000D and U+0020.
Interior whitespace, Unicode content and non-ASCII whitespace are retained.
Empty/all-whitespace inputs yield an empty string. Existing split/array/join
primitives avoid repeated string indexing and concatenation: O(n) time/storage.
The fixed whitespace constant uses existing JSON decoding; no new runtime syntax
or builtin is introduced. Fufu's tests exercise actual source/OXBC module loading.

The `.ox.data` archive is opaque fixture data, outside language discovery.
The live `stdlib/strings.ox` remains a checked language input.
`archive/stdlib/strings.ox.data` retains the original 2345-byte stub, SHA-256
`6ecb6b4c96acd0e788bf49db9a480c930d4235e317af8ffc596bbe8868fb65fd`, Git blob
`9f81e678fa63bd44b3387db2e1f06e2279b2d72f`.
`derive.py` derives a named predecessor helper from the pinned original, changing
only capture's fixed strings storage read. The accepted-view descriptor, its row,
source seal, every source generation and all old helpers/proofs remain unchanged.
All 2724 historical view bodies retain their original logical paths and bytes.

## Mechanical fixture

`retained.py` authenticates `mechanical-inputs-v1.json` against a fixed literal pin.
Every access reads and hashes the actual bytes from exactly one declared storage
path. Unchanged inputs are read live; only changed bodies use fixed archives.
No environment override, fallback, unlisted paths or process-global monkeypatch
is used. The manifest is an identity list, not a materialized repository copy.

The old scheduling producer still reads its complete 1048 data bodies and runs
original hashing, membership, transformation and relocation checks. Its five Git
inputs and ten additional historical-test comparisons retain the original
`31f49568b1142dd58d80eef8f71faeeb99f32ba5` association; raw commit and eleven raw
tree nodes prove all fifteen paths/blobs without runtime Git or network access.
The six producer helper identities are distinct from test harness code. Private
module aliases prevent late `import ci` from reaching ambient repository code.
All executed retained code is authenticated before adaptation/compilation; this
is a fixed reviewed adapter, not a general Python sandbox.

Only the two original scheduling mechanical methods use the adapter. The shallow
checkout control's old-generation comparison uses the same adapter and additionally
checks active outputs; fetch/tree/shallow assertions remain live. The original
five hosted mechanical tests run against fixed inputs/outputs at their normal
logical filenames. Their 92-case count does not execute the 92 hosted controls.
Other scheduling discovery, receipt, shard/join and current inventory admission
continue to exercise live code. New active controls reject cross-generation pins.

## Active integration

`activate.py` emits a finite output map: dispatcher/package pins, retained test
facades, parser authority, Unit1 support, public lifecycle loader/proof, hosted
bootstrap/facade and the active input inventory. Public build's two explicit
loader sites select the new lifecycle helper; old lifecycle helper/proof remain
intact. Normal hosted bootstrap performs genuine current preflight/admission.
Native source capture includes this namespace, so relocated dependencies survive
source snapshotting. No workflow, compiler source roster, historical source
manifest/checkpoint/proof or execution-qualification label is rewritten.

`inputs-stdlib-trim-v1.json` and active `inputs.json` include live strings, fufu
tests, all helper/archive/derivation/control files and active consumer outputs.
The historical `inputs-scheduling-v1.json` remains unchanged. Integration helper
identities are also recorded by the existing exact-head gate. This source-only
change does not itself grant hosted/native execution qualification.

## Reproduction

Use the installed toolchain and run commands from the repository root.
`review_inputs.py` needs the two fixed Git commits locally; it never fetches.
Its output map is finite and reproduces only named metadata and changed archives.
The metadata literal pin in retained.py is a review boundary, not auto-updated.

```sh
python3 -B tests/qualification/stdlib_trim_storage_v1/review_inputs.py --check
python3 -B tests/qualification/stdlib_trim_storage_v1/derive.py --check
python3 -B tests/qualification/stdlib_trim_storage_v1/activate.py --check
python3 -B tests/qualification/stdlib_trim_storage_v1/test_storage.py -v
python3 -B tests/qualification/stdlib_trim_storage_v1/test_retained.py -v
python3 -B tests/qualification/stdlib_trim_storage_v1/test_active.py -v
cargo test --offline --test stdlib_strings --test legacy_modules
```

To regenerate reviewed outputs, use `review_inputs.py --write`, followed by
`activate.py --write`; run twice and require identical bytes. New output identity
manifests are evidence only, not historical qualification replacements.
Archive controls reject changed/missing/oversized/executable files, symlinks,
hardlinks and directory aliases. Descriptor, row, mapping, coherent rehash,
unknown reads/imports, helper substitution and wrong commit associations reject.
The complete source inverse and historical view are checked, including nested
historical preflight and post-operation closure. Final hosted/cross-platform gates
remain distinct from these local focused checks.
