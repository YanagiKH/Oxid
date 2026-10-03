# Fixed-array Unit2E producer-native CI admission

This adapter admits and preserves the existing ignored owned-native prefix once
per profile. It is a producer-only boundary. Independent final-verifier and
combined evidence integration remain a separate next step. The existing source
binding must be refreshed separately after the final source freeze; stale pins
fail the official preflight. No synthetic fixture or earlier producer log can
satisfy operational admission.

## Run and preserve

On the pinned Linux x86_64 native CI image, after the existing locked dependency
fetch:

```sh
python3 -B scripts/verify_owned_array_native.py run \
  --repo "$PWD" --output "$RUNNER_TEMP/fixed-array-unit2e/producer" \
  --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" \
  --cargo "$(rustup which cargo)" --rustc "$(rustup which rustc)" \
  --cargo-home "${CARGO_HOME:-$HOME/.cargo}" \
  --llvm-bin "$OXID_LLVM_BIN" --profile both

python3 -B scripts/verify_owned_array_native.py package \
  --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" \
  --output "$RUNNER_TEMP/fixed-array-unit2e-upload" \
  --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" \
  --producer-step-outcome "$UNIT2E_PRODUCER_OUTCOME"
```

Output roots must be fresh, external and nonoverlapping. There is no resume,
synthetic operational mode, skipped-preflight switch, or count override. Running
Python with optimization is unsupported. A single-profile local run is allowed;
CI packaging requires both debug and release from the same invocation.

The controller verifies exact HEAD/tree, committed compiler/build/adapter inputs
and the existing official source preflight before any build or tool version
execution. It records current-source manifest, reviewed-source head and
source-only tree separately from CI head and event SHA. Original workflow run,
attempt, job and workflow identity remain distinct from checked-out workflow
bytes. It never refreshes a source pin or relaxes an existing gate.

Rust must be release 1.99.0, full commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, host
`x86_64-unknown-linux-gnu`. LLVM must be 19.1.7. Tool receipts retain invocation
paths, resolved target hashes and original versions, including the `ld.lld`
invocation basename. The selected Rust standard library and C/C++/AR tools are
also recorded. Every version/build/list/native child receives the exact admitted
`OXID_LLVM_BIN`; `LD_LIBRARY_PATH` remains unset for the pinned Debian recipe.
Other ambient compiler flags, wrappers and loader injection controls are omitted.

The explicit child environment preserves the actual HOME and selected CARGO_HOME
cache. Unsupported Cargo-home, ancestor or untracked repository configuration is
rejected by presence/type metadata before Cargo; its contents and credential
files are never read or archived. Builds use `--locked --offline --jobs 2` and
`CARGO_INCREMENTAL=0`. Existing target/cache contents are not deliverables.

Each build retains original Cargo JSON and resolves exactly one test executable.
The copied ELF is then listed once and executed once with the broad prefix.
Every child owns a new process group. Surviving descendants after exit 0, exit 7,
exceptions, signals or timeouts reject admission; bounded cleanup reaps only that
owned group before logs are hashed. Original direct-child status is retained
separately. First failure stops later profiles and propagates nonzero.

## What producer evidence means

The roster is exactly 16 tests, including six array families. Every stdout
completion is attributed by name with original byte/line spans. Only the named
source-resource test admits its exact three-line stdout payload. The six full
family summaries must each occur exactly once in separate original stderr.

The six array tests assert 7,058 ELF executions per profile in their original
summaries. The 6,793 reference comparisons are derived from source control flow.
Neither number claims retained per-process receipts or independent replay.
Intermediate effect traces are reference observations; the native tests check
final results and exact failure diagnostics.

Each profile requires 1,307 array artifact names: 524 executable ELF64
little-endian x86_64 files, 524 compiled UTF-8 LLVM modules, and 259 original
guarded production LLVM modules. Missing, extra, nested, nonregular or symlink
members fail. Original inherited artifacts remain retained with an inherited
role; unchanged inherited tests can overwrite their own names, so this is not a
claim to preserve every historical inherited scratch object.

## Full and compact retention

Packaging checks the actual Actions producer outcome and revalidates source,
tools, original command status/logs, profile bindings, copied ELFs and all artifact
bytes. Stable copies use bounded reads, anchored no-symlink traversal and
before/after identity checks. Any traversal or I/O failure makes packaging
incomplete. The archive and its membership manifest use the same captured bytes;
archive members are read back and hashed before upload.

The full `fixed-array-unit2e-evidence.tar.gz` retains every safely captured
producer member, including copied test binaries, original modules, array ELFs,
inherited artifacts, source inputs and complete original logs. No deliverable is
pruned for size. `fixed-array-unit2e-index.json` is a compact convenience summary.
`fixed-array-unit2e-compact.tar.gz` carries actual audit bodies with this explicit
roster:

- Root invocation, terminal result/failure and full evidence-membership receipts
- Source identity, original authority/package/current-source manifests, official
  preflight receipt/plan and original preflight command/logs
- Toolchain identity, every original tool-version command/stdout/stderr, selected
  standard-library directory command and standard-library hashes
- Each profile's result/failure, roster, complete artifact manifest, original
  build command/stdout/stderr and complete named-test list/run command/stdout/stderr
- Full and compact archive-membership manifests

The compact archive is capped at 32 MiB for remote transport. Exceeding that cap
fails packaging explicitly while preserving the full archive; nothing is
truncated to force a pass. Both archives have SHA-256 sidecars. The workflow
always attempts packaging and separate strict compact/full uploads. Forced
runner termination can prevent those steps; no receipt then claims completion.
A complete producer package still sets `complete_unit2e_qualification: false`.
Independent debug/release receipts and their complete artifact closures must be
integrated under their own frozen schema before complete Unit2E qualification.

## Synthetic controls

```sh
python3 -B -m unittest discover -s scripts -p test_owned_array_native.py -v
```

These bounded tests cover exact selection/attribution, malformed and stale
receipts, tool-selection traps, metadata-only config admission, nonzero and live
descendant exits, missing/changed artifacts, stable partial snapshots and archive
bytes. Constructed successful receipts are explicitly synthetic fixtures, never
real Rust/LLVM execution evidence. Cargo/LLVM corpus execution belongs to the
later coordinated exact-head rehearsal after source bindings and integration
are frozen.
