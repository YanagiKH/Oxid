# Complete independent Unit2D replay, runner successor v2

This runner wires the frozen reviewer inputs into a disposable source snapshot.
It does not register those inputs in the user's Cargo worktree. Its default scope
is exactly 17 independent functions: seven frozen ordinary functions plus one
fresh source-content marker, eight ignored native families, and the ignored
physical observer family. It also compares all 90 old LLVM modules and the
exporter's exact TSV, checks 379 bounds sites and 28 pointer phis, and binds all
336 ELF artifacts and 1,210 source-free execution receipts.

These are regression gates for the private native consumer. They do not enable
public source arrays and do not constitute all repository, release, or Unit2D
acceptance checks. Existing ordinary production tests and ignored production
native suites are outside this runner's declared scope.


## Expectation and inventory provenance

Correct language values, diagnostic kinds/spans, fuel schedules, raw-authority
denials, shared-alias results and MIN/MAX results come from the independently
frozen Rust/model inputs and `expectations/supplement-v1.json`. The physical
manifest specifies exact expected statuses/stdout/stderr for 18 positive cases
and 11 targeted mutants. Those are source-derived expected results, unchanged
by this runner. The 90 old LLVM hashes and exact TSV are a separately built PR28
comparison baseline; 90 is also the exporter's 9 builders × 2 guard choices ×
5 source-map choices. Its 59 distinct hashes are an observed historical fact.

The 336 ELF, 1,210 runtime and 3,360 tool-receipt totals are coverage targets
assembled from separately passed historical inventories: v1 331/1,205/3,310 plus
supplement 5/5/50. They are not semantic or fuel oracles, and they do not claim
that the combined full suite has previously run. Likewise, the 275 exact
structural sidecar names, 379 parsed bounds sites and 28 pointer phis were
observed in the frozen v1 replay and text audit. They constrain complete replay
of those frozen inputs; correct bounds dominance and phi predecessors are
checked separately by the independent structural reader. The supplement's
164-diagnostic-byte assertion and progress beyond 164+128 expansions were added
after the original observation and are explicitly a resource/input-completeness
check, not an independent pre-observation semantic or resource oracle.
Missing even an empty
construction sidecar fails the exact-name inventory check.

The 17-function roster comes from frozen Rust declarations plus one generated
marker (eight ordinary and nine ignored native). Pinned toolchain versions, jobs
and platform are the replay environment contract. SHA/ELF/libtest/schema checks
are format/protocol invariants. `expectation-provenance.json` records these
categories with their source paths; `qualification-v2.json` preserves the
separate historical runs and explicitly marks fresh combined replay pending.

## Requirements

- Python 3.10 or later, standard library only, without `-O`
- A local clean Git checkout containing the requested full commit ID
- Linux x86_64 for build/native execution
- Already installed Rust `1.99.0` and LLVM `19.1.7`
- An already populated Cargo dependency cache; Cargo always uses `--locked
  --offline`, jobs `2`, and `CARGO_INCREMENTAL=0`
- Enough space for a fresh source snapshot, external Cargo target, and retained
  evidence; the runner never deletes evidence or executable artifacts

The runner performs no fetch, installation, remote write, or checkout mutation.
It rejects tracked/untracked dirty input, links/submodules in the Git tree, and
archives modified by `export-ignore` or `export-subst`. Ignored files such as an
existing Cargo target are not copied. An explicit commit may differ from the
checkout's clean HEAD; both identities are recorded. If the input is dirty,
commit or stash the edits yourself, or supply another clean checkout.

The supplement source identity is
`e087e232b4a9885447352b69b4279e5c30978d16`, tree
`0a4e1c4c3a8b83165d2b2eec01e3a82d3780258a`. The preserved v1 qualification
records the earlier equal-tree pair `8251d2a96dd78e26ed63bd25ec9353b111cc28b5` /
`3882ccd174e361ef1a1dcfa030e12740cb139d93`. A later repository commit
may also be supplied to run these same frozen regressions against that exact
snapshot. A later source identity is never described as the historical source.

## Complete replay

Create a JSON file naming the actual trusted official LLVM tool installations.
Paths may point to symlinks supplied by that installation; preserve the
`ld.lld` invocation name because LLVM's linker is a multicall executable.

```json
{
  "llvm-as": "/opt/llvm-19.1.7/bin/llvm-as",
  "opt": "/opt/llvm-19.1.7/bin/opt",
  "clang": "/opt/llvm-19.1.7/bin/clang",
  "ld.lld": "/opt/llvm-19.1.7/bin/ld.lld"
}
```

Run from a checkout containing these scripts and the packaged fixture inputs:

```sh
python3 -B scripts/replay_fixed_array_unit2d.py \
  --repo /path/to/clean/Oxid \
  --commit e087e232b4a9885447352b69b4279e5c30978d16 \
  --output /path/to/new-external-replay \
  --profile debug \
  --rust-bin /path/to/rust-1.99.0/bin \
  --cargo-home /path/to/populated-cargo-home \
  --trusted-tools /path/to/trusted-llvm-tools.json
```

`--profile debug|release` selects the Cargo profile; new runs default to `debug`.
Run both profiles with separate fresh output directories. The selected profile
is part of the source marker and final verification record. Release uses
`cargo test --release`; it does not reuse the debug target or binary.

`--output` must not exist and must be outside both the supplied source checkout
and fixture directory. The Rust directory must contain the pinned `cargo`,
`rustc`, and `rustdoc`; no rustup installation is initiated. The LLVM map must
contain exactly the four shown tool names. If a producer suite also needs
`llvm-readobj`, provide a separate four-key JSON map for this independent runner.
Versions, resolved file contents,
argv, working directories, statuses, stdout and stderr are recorded. Tool
contents are rechecked before the transparent wrapper calls them.

`--fixtures /path/to/fixed_array_unit2d_independent` changes input discovery.
`--input-manifest /path/to/reviewed-manifest.json` changes the exact input
manifest; by default it is `replay-inputs-v1.json` in the fixture directory.
The successor manifest at the reserved `replay-inputs-v1.json` path pins all
36 package1c inputs plus the seven separately versioned supplement-v1b inputs,
43 files total. It selects `qualification-v2.json` and the exact combined
`sources/reviewer-array-native-v3.rs`; the original `qualification.json` and
combined v1 module remain unchanged historical inputs. The previously delivered
14-test runner artifact remains preserved separately. Future supplemental tests
or revised oracles require a reviewed versioned integration,
including an updated inventory and evidence counts. They are not discovered and
run silently.

## Source-only preparation and phases

For a local review before repository integration, stage the 36 base files and
seven supplement-v1b files in one new immutable fixture directory. The five-path
runner deliverable does not duplicate those 43 source files. Set `BASE_FIXTURES`
to the base package's `payload/tests/fixtures/fixed_array_unit2d_independent`,
`SUPPLEMENT_FIXTURES` to the supplement-v1b directory at that same relative path,
`RUNNER_MANIFEST` to this successor's `replay-inputs-v1.json`, and `NEW_FIXTURES`
to a directory that does not exist. This staging command copies only manifest
members after verifying exact lengths and hashes:

```sh
python3 -B - "$BASE_FIXTURES" "$SUPPLEMENT_FIXTURES" "$RUNNER_MANIFEST" "$NEW_FIXTURES" <<'PY'
import hashlib, json, pathlib, sys
base, supplement, manifest_path, output = map(pathlib.Path, sys.argv[1:])
manifest_bytes = manifest_path.read_bytes()
manifest = json.loads(manifest_bytes)
assert len(manifest["files"]) == 43
output.mkdir(parents=True, exist_ok=False)
for row in manifest["files"]:
    relative = pathlib.PurePosixPath(row["path"])
    assert not relative.is_absolute() and ".." not in relative.parts
    candidates = [root / relative for root in (base, supplement) if (root / relative).is_file()]
    assert len(candidates) == 1, row["path"]
    data = candidates[0].read_bytes()
    assert len(data) == row["bytes"]
    assert hashlib.sha256(data).hexdigest() == row["sha256"]
    target = output / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
(output / "replay-inputs-v1.json").write_bytes(manifest_bytes)
print("Staged 43 verified frozen inputs plus the exact replay manifest:", output)
PY
```

Pass `--fixtures "$NEW_FIXTURES"` to the runner; it discovers the staged manifest
automatically. Keep that directory unchanged for the entire run and any resume.
It must be disjoint from the replay output. No file is read from the separate
base/supplement packages after staging. A fully integrated repository already
provides the single combined fixture root and needs no staging step.

To inspect the declared test names without any preparation or execution:

```sh
python3 -B scripts/replay_fixed_array_unit2d.py --list
```

To materialize and bind the source without invoking Rust, LLVM, or an ELF:

```sh
python3 -B scripts/replay_fixed_array_unit2d.py \
  --repo /path/to/clean/Oxid \
  --commit FULL_40_CHARACTER_COMMIT \
  --output /path/to/new-external-replay \
  --phase prepare
```

Phases are cumulative: `prepare`, `build`, `ordinary`, `native`, `physical`,
`verify`. The default `full` runs all of them. `build` builds the fresh binary,
checks its complete independent/ignored inventory, and actually runs the marker.
`ordinary` runs the other seven ordinary tests and checks old IR. `native` runs the
eight native families, including the shared-alias/extreme-value supplement and
the 18 chain modules needed by `physical`.
`physical` runs the Python observer's text tests, prepares and reconstructs the
29 physical cases, creates a byte-identical `manifest.tsv` alias of
`harness.tsv`, then runs and compares all 18 positive and 11 expected status-1
mutant outcomes. `verify` checks counts, tool receipts, structural controls,
source/binary identities and retained artifacts.

Continue a successful partial phase with the same output:

```sh
python3 -B scripts/replay_fixed_array_unit2d.py \
  --resume --output /path/to/new-external-replay --phase full \
  --rust-bin /path/to/rust-1.99.0/bin \
  --cargo-home /path/to/populated-cargo-home \
  --trusted-tools /path/to/trusted-llvm-tools.json
```

Once toolchain paths are recorded, resume reuses and verifies those exact paths
and the original trusted-map file. Conflicting profile, commit, checkout, tool-map,
Rust directory or Cargo-home arguments are rejected. A profile omitted on resume
means the original bound profile. Completed phases
are verified and retained, not rerun. Resume requires the original checkout to
retain its captured HEAD and clean status, and requires the original fixture
directory and input manifest to retain their exact contents and membership. It
compares exact membership and bytes of the latest sealed evidence closure,
including top-level final reports/bindings, before continuing or acknowledging a
completed full run. Read-only Git resume checks have separate retained receipts
under `resume-checks-*` outside the sealed evidence directory. Failed/interrupted outputs are never reused
or overwritten: fix the cause and choose a new output directory. A partial phase
is reported as partial, and a prepared-only directory does not claim any replay
or native qualification. Run directories cannot be relocated for continuation.

## Output and identity contract

```
OUTPUT/
  source/                    isolated full Git archive + test-only append
  inputs/                    immutable copied frozen package inputs
  target/                    fresh external Cargo target
  state.json                 phase progress; passed only after verify
  evidence/
    source.tar               exact archived source
    original-source.json     every original SHA-256, Git blob ID, length, mode
    input-manifest.json      exact copied frozen input manifest
    expectation-provenance.json source-oracle versus historical-inventory origins
    source-binding.json      source tree, original hashes, marker, prepared hashes
    binary.json              fresh Cargo artifact, executable hash, source binding
    bin/oxid-unit2d-tests     retained newly built executable
    test-inventory.json      full and scoped ordinary/ignored inventories
    commands/                every top-level argv/status/stdout/stderr receipt
    tool-receipts/           unchanged frozen logger receipts
    tool-captures/           deduplicated referenced compiler-workspace artifacts
    native/                  emitted LLVM and native ELF artifacts
    executions/              source-free argv/status/stdout/stderr receipts
    old-ir/                  90 modules and exact inventory TSV
    physical/                preserved source/observer/mutant modules and receipts
    phase-*-artifacts.json   exact cumulative evidence closure and hash seals
    verified.json            final scope and verified identities/counts
```

The marker binds the original source manifest, frozen input hashes, appended
controls, runner/capture scripts and a fresh nonce. Its exact test name and
printed digest must match; its compiled Cargo source directory must match the
new isolated snapshot. The selected Cargo artifact must be freshly compiled
within the new target. Its retained copy is hashed before and after every test.
Every independent test runs by its full `--exact` name and must report exactly
one passed test with none ignored. No pre-existing executable can be supplied.
The historical executable hash in `qualification.json` is reference information;
fresh instrumented binaries are expected to have their own verified hashes.

The original `native_tests.rs` bytes are preserved as an exact prefix; only the
four frozen controls/exporter, module declaration and fresh marker are appended.
The combined reviewer module is copied unchanged. Full source/input inventories
are rechecked around execution. The existing Rust harness removes its private
temporary compiler workspaces; the outer Python wrapper retains only explicitly
referenced `.ll`, `.bc`, `.c`, and `.o` files under the runner's temporary root,
deduplicated by SHA-256, before that cleanup. It does not archive arbitrary tool
arguments or system libraries. Successful native ELFs are preserved by the
existing frozen harness. The runner never cleans retained evidence or targets,
including after failures.

Keep the generated source, Cargo target, LLVM, native executables and receipts
outside the repository package. Repository deliverables are these small scripts,
the frozen input manifest and this documentation.

## Lightweight source-only tests

```sh
python3 -B -m unittest discover -s scripts -p test_replay_fixed_array_unit2d.py -v
```

These tests also reject missing structural sites/sidecars, zero/incomplete
execution totals, added evidence members, altered final reports, changed original
checkout HEAD/clean status, and changed original fixture/manifest inputs.
They create synthetic local Git repositories and fake ELF bytes (never
executed), and use Python subprocesses to test failure receipts. They check dirty
source rejection, source/hash binding, path escapes, exact test inventory, false
zero-test passes, old LLVM/TSV mismatch, expected mutant results and preservation
of failed command outputs. They do not establish a real Rust/native replay.
