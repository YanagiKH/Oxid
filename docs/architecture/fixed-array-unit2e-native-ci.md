# Fixed-array Unit2E native CI admission and retention

`scripts/verify_owned_array_native.py` admits the existing producer-owned native
prefix, supervises the frozen independent Unit2D replay, and packages their
separate original evidence. These are private qualification boundaries. They do
not enable either public array gate or establish whole-repository acceptance.
The final published head still requires all applicable CI checks.

## One immutable run

Use the pinned Linux x86_64 native CI image after the existing locked dependency
fetch. Each output root must be fresh, external and nonoverlapping. The workflow
checks out the exact PR head, records the event SHA separately, and executes one
producer operation followed by one complete independent invocation per profile:

```sh
python3 -B scripts/verify_owned_array_native.py run \
  --repo "$PWD" --output "$RUNNER_TEMP/fixed-array-unit2e/producer" \
  --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" \
  --cargo "$(rustup which cargo)" --rustc "$(rustup which rustc)" \
  --cargo-home "${CARGO_HOME:-$HOME/.cargo}" \
  --llvm-bin "$OXID_LLVM_BIN" --profile both

for profile in debug release; do
  python3 -B scripts/verify_owned_array_native.py independent \
    --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" \
    --output "$RUNNER_TEMP/fixed-array-unit2e/independent/$profile" \
    --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" \
    --profile "$profile"
done
```

CI uses separate independent debug/release steps so their actual Actions outcomes
remain distinct from producer status. The controller invokes the existing
`replay_fixed_array_unit2d.py` once per profile with its default full phase,
`--repo`, `--commit`, fresh `--output`, admitted `--rust-bin`, a four-key absolute
LLVM-path map, `--profile` and the admitted `--cargo-home`. It never runs the six
cumulative phases separately or resumes old evidence. Original outer invocation
logs, status and binding receipts live in sibling `<profile>-invocation/`
directories, outside the runner's immutable state and seals.

There is no operational synthetic mode, skipped-preflight switch, count override
or overwrite/resume flag. Optimized Python is rejected. A producer-only local
single-profile run is supported; CI packaging requires producer debug and release
from the same invocation, plus both independent profiles for combined completion.

## Source, tool and process admission

Before a build or tool version child, the producer verifies exact HEAD/tree,
clean tracked bytes, committed compiler/build/adapter input membership, and the
existing official source-binding preflight. Current-source manifest,
reviewed-source head and source-only tree remain distinct from CI head/event and
executed workflow identity. Stale pins fail; this controller never refreshes them.

Rust must be release 1.99.0, full commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, host
`x86_64-unknown-linux-gnu`. LLVM must be 19.1.7. Receipts retain exact invocation
paths, resolved target-byte hashes and original versions, including the `ld.lld`
basename. Selected standard-library files and C/C++/AR tools are also bound.
Every controller-managed version/build/list/test/independent-launch child gets
the admitted `OXID_LLVM_BIN`; `LD_LIBRARY_PATH` stays unset for this Debian recipe.
The frozen independent runner then uses its documented logger links, whose
original LLVM receipts must resolve to the same admitted four tools.

The explicit child environment preserves actual HOME and selected CARGO_HOME.
Ambient compiler flags, wrappers and loader injection controls are omitted.
Unsupported Cargo-home, ancestor or untracked repository configuration is
rejected by presence/type metadata before Cargo. Credential files are never
opened or copied, including tracked Cargo credentials or manifest-selected
credential names. Source membership and Cargo discovery are checked again at
boundaries. The independent source clone's external Cargo discovery locations
receive the same metadata-only checks. Builds use locked offline dependencies,
two jobs and no incremental compilation.

Each child starts a controller-owned process group. On every exit path the
controller checks and stops all members remaining in that group, reaps adopted
members of that group, and only then seals stream hashes. Surviving members after
exit 0 reject admission; exit 7 retains its original status even if receipt
sealing also fails. Timeout, signal and partial failure remain nonzero. Deliberately
detached processes are outside this bounded contract; it is not a general
sandbox or descendant-containment claim. The reviewed supported launch sites do
not request detachment. A failure stops later operations; unstarted profiles
never acquire a pass.

## What the two kinds of evidence mean

The producer builds and copies one test executable per profile from original
Cargo JSON, lists it once, and runs the broad prefix once. Its exact roster has
16 tests, including six array families. Every stdout completion is attributed by
name and original byte/line span. Only the named source-resource test admits its
exact three-line stdout payload. All six full family summaries must occur once
in separate original stderr.

Producer summaries assert 7,058 ELF executions per profile. The 6,793 reference
comparisons are derived from source control flow; neither number represents
retained per-process receipts. Intermediate effect traces are reference
observations, while native checks prove final results and exact diagnostics.
Each profile preserves exactly 1,307 array files: 524 executable ELF64
little-endian x86_64 files, 524 compiled UTF-8 LLVM modules and 259 original
guarded production modules. Missing, extra, nested, nonregular or symlink array
members fail. Inherited artifacts are retained under their own role; unchanged
inherited tests may overwrite their own names, so this is not a complete archive
of every historical inherited scratch object.

Independent results stay separate. Admission joins schema-1 state, the exact
prepare/build/ordinary/native/physical/verify completion sequence, five fixed
phase seals, source binding, marked binary, frozen inputs/tools and original
individual test results. The final seal is an exact file/directory/symlink list.
Only the eight frozen logger symlinks are allowed, with exact raw targets; they
are never followed by the exporter. Traversal holds and verifies ancestor
identities, allows unrelated sibling activity, and rejects root replacement or
I/O failures. All five seals and the exact input inventory are rechecked at
terminal admission, in addition to state and evidence bytes.

The independent frozen coverage inventory includes eight ordinary tests, nine
native/physical families, 336 ELF artifacts, 1,210 source-free execution receipts
and 3,360 LLVM command receipts. Physical evidence records 29 cases, including 11
expected mutant failures. Original old-IR, structure, physical and supplement
comparison bodies remain available. These historical coverage counts are not
semantic oracles; correctness comes from the frozen assertions, expectations and
original comparisons. They never replace or merge with producer counts.

## Complete and partial exports

At the end of the native job, packaging always receives the three actual step
outcomes:

```sh
python3 -B scripts/verify_owned_array_native.py package \
  --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" \
  --independent-root "$RUNNER_TEMP/fixed-array-unit2e/independent" \
  --output "$RUNNER_TEMP/fixed-array-unit2e-upload" \
  --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" \
  --producer-step-outcome "$UNIT2E_PRODUCER_OUTCOME" \
  --independent-debug-outcome "$UNIT2E_INDEPENDENT_DEBUG_OUTCOME" \
  --independent-release-outcome "$UNIT2E_INDEPENDENT_RELEASE_OUTCOME"
```

Stable copies use bounded reads, no-symlink file traversal and pre/post identity
checks. The manifest and archives use those same captured bytes. Archive readback
and compressed-byte hashing share one stable file descriptor, followed by a
terminal comparison against those verified records. Gzip/tar metadata is fixed
so the same captured bytes produce the same archive. Any missing, changed,
unreadable, failed or skipped evidence makes the index incomplete and packaging
nonzero while retaining safely captured originals. A fresh export never replaces
an older accepted index.

The full `fixed-array-unit2e-evidence.tar.gz` preserves producer evidence and each
independent runner's declared evidence closure, inputs, state and outer invocation
receipts. It includes copied test binaries, native ELFs/IR, captured compiler
inputs, original streams, source.tar and all comparison/phase bodies. The frozen
runner excludes `evidence/temporary/` scratch. Its materialized source checkout
is represented by the retained source archive and original/prepared-source
bindings. A target cache is omitted only after validating the required copied
marked binary and its source/state binding; otherwise surviving partial target
artifacts are retained. No deliverable is pruned for size.

The separate compact archive carries actual bodies, not just their hashes:

- Producer invocation/results, source/tool identities and original authority,
  package/current-source manifests; preflight and tool-version command streams
- Every producer profile result, roster, artifact manifest, Cargo build streams
  and original named-test list/run receipts and streams
- Independent state, all phase manifests, source/input/provenance/tool/binary/test
  inventories, verifier/comparison/structure/execution/artifact bodies and inputs
- Every independent ordinary/native/physical command receipt and stream, every
  source-free execution receipt/stream, and both LLVM logger layers' receipts and
  streams; top-level physical expected/input/harness bodies
- Separate independent invocation receipts/streams and exact full/compact member
  manifests, including an explicit full-only omission roster

Large binary, IR, insertion and captured-input payloads stay in the full archive.
The compact archive is capped at 32 MiB compressed. Exceeding the cap fails
explicitly and preserves full evidence; no required body is truncated to fit.
Both archives have SHA-256 sidecars and separate always-run uploads. The quality
job separately retains the new source observer-control JSON/stdout/stderr bodies
through three exact additive globs and its own relocated-capsule audit helper.
Forced runner termination can prevent an upload; no receipt claims otherwise.

`fixed-array-unit2e-index.json` remains the convenience summary. Producer-only
packaging always sets `complete_unit2e_qualification: false`. Combined completion
requires producer and both independent profiles to succeed with matching exact
head/run/tool/source identities. It does not replace unrelated CI gates.

## Audit downloaded compact evidence

From a checkout containing this controller and the frozen replay-schema helper:

```sh
python3 -B scripts/verify_owned_array_native.py audit-compact \
  --index /path/to/downloaded/fixed-array-unit2e-index.json \
  --archive /path/to/downloaded/fixed-array-unit2e-compact.tar.gz \
  --expected-head FULL_PR_HEAD --event-sha ORIGINAL_EVENT_SHA
```

This checks the archive checksum, exact selected-body roster and hashes,
full/compact member joins, original source/head/profile/tool/binary commitments,
named-test results and retained comparison/runtime bodies. It operates on the
downloaded archive and local schema helper; it does not restore or open hosted
absolute paths. Downloaded outer-file permissions may differ; archive member
modes remain bound. Decoded body input is bounded at 256 MiB. The report explicitly
identifies full-only binaries/IR as indexed commitments that were not retrieved
or replayed. The watcher must also match the index's run/attempt identity to the
actual Actions run and verify successful hosted uploads and applicable checks.

## Bounded controls and final execution

```sh
python3 -B -m unittest discover -s scripts -p test_owned_array_native.py -v
python3 -B -m unittest discover -s scripts -p test_unit2_observer_artifact_retention.py -v
```

These are synthetic fixture/process/file controls, not Rust/LLVM qualification.
Successful constructed receipts are explicitly synthetic. Historical debug
replay supplies a read-only schema fixture, not final-head execution evidence.
The final coherent source/workflow checkpoint still needs one fresh producer
both-profile operation, one full independent operation per profile, combined
packaging and downloaded-body audit, followed by exact-head hosted CI.
