# Dormant fixed-array Unit3A historical replay

This package preserves the source-only observer and external comparisons used to
qualify the dormant Unit3A frontend. It provides historical reconstruction and
replay instructions; packaging it does not execute the corpus or qualify the
current repository head. Public array grammar and production raw array gates
remain closed. Type resolution, lowering, ownership execution, native source
support and public activation are outside this checkpoint.

The executed acceptance implementation is `components/observer-v3`. The corrected
external mutation runner is `components/sensitivity-v5/sensitivity.py` and imports
the unchanged v3 comparator. Their bytes are preserved separately. `inputs`
contains original source, assembly, component and request-binding manifests.
Those lineage manifests also name historical files deliberately omitted from
this selected-file package, including the old planning README and path-dependent
request TSVs. `package-manifest.json` is the complete roster of files actually
published here. This README, `verify_inputs.py` and its bounded
`test_request_identity.py` controls are packaging artifacts.

## Established historical results

Linux x86_64, Rust 1.99.0: 58 frontend cases and six first-excess boundary cases
passed in debug and release. Four lowered output-limit controls in each profile
returned explicit incomplete errors. Seventeen focused controls passed in each
profile; all 68 source observations/error artifacts matched between profiles.
The corrected external sensitivity runner rejected all 18 scoped mutations.
A separate ordinary debug suite passed 900 tests with 27 ignored across 17
suites. The ordinary full release suite was not run. The 29 later resolver/type
cases and two effect counterexamples remain deferred; parse success or temporary
E0500 admission rejection never counts as their semantic pass.

The retained comparison reports bind individual observation hashes. Raw compiler
executables and full observation bodies are not included in this small input
package. Original binary hashes identify retained historical executables; a new
build gets its own binary hash and command/toolchain receipt and must not be
represented as byte-identical without checking it. `evidence` contains unchanged
technical reports and focused-test logs. `historical/failure-summary.json`
records the initial failures and original artifact identities.

The observer uses the real candidate loader, AST, checked type queries, actual
source allocation identities, ExprIds and child edges. Source expectations
remain external to the compiler. Requested structural projections, source roles
and exact diagnostics are compared independently. Unrequested generic expression
internals are production-validated, not claimed as completely independently
compared. Temporary resolver fence tests are separate from the exact-origin
frontend corpus. No HIR or execution witness enters this observer.

## Resource and completion contract

The request format is five tab-separated fields: case label, `single` or
`project`, source directory, row limit, byte limit. The helper reads one opened
request descriptor through a hard 256 KiB plus one detection-byte limit and
rejects lengths at or above 256 KiB. It rejects duplicate labels. Requests contain
no expected AST, type, diagnostic or runtime output.

Each output row is measured with checked arithmetic before a fallible exact
reservation. Hard maxima are 200,000 rows and 16 MiB serialized UTF-8 bytes,
including newlines; requests may only lower them. The success footer records
completed phase and preceding row/byte totals. An incomplete run emits a separate
`.error`, never a successful partial `.jsonl`. The output directory must be absent
and each artifact is created once. All comparators require exactly one ordinary
artifact per expected case and reject missing, unknown or simultaneous
success/error artifacts. The unchanged 18 mutation controls operate on copied
observations outside the compiler.

Auxiliary bounds are separate: source-map copied text/path bytes are preflighted
against 16 MiB; store-target flags reserve the bounded expression count;
aggregate query results are limited to 200,000 and 16 MiB requested payload;
diagnostic rendering gets a conservative byte preflight after original span
validation. These are requested-storage bounds, not RSS or universal OOM claims.
Source/token/node limits and native limits are unchanged.

## Historical source identity

The production base is `d582d1dca2a26eb3230632a921f9e8166c39d5c0`, with tree
`a826c1782dac19b8c34bdafb0202240e5d414719`. Its 1,088 files plus the preserved
13-file patch reconstruct the complete 1,092-file checkpoint4 source closure.
All nine replaced base bodies match the patch's old-body hashes; four paths are
new. `inputs/checkpoint4-manifest.json` binds every resulting file. The preserved
patch SHA-256 is
`0f9dad255cbcc73556bfd0dc4c64f8bbb1c3c0b13f6fa42e48ec86e95284b899`.

The historical compressed archive SHA-256 is
`aace15bc61711ce9f7731c435f5618eb9b6a1118da4f226e9172b4b053891624`.
Git reconstruction establishes the complete file closure, not byte identity of a
new tar/gzip container. Archive metadata and compression can differ. Installing
exactly the v3 observer and the three-line test-only module suffix yields the
1,093-file assembly bound by `inputs/assembled-v3-files.json`, SHA-256
`f091756f3f6c9a38a8896bec6040673a74480d4c1f45bd2e51b09011d6857198`.

The observer header contains the historical archive identity. That string alone
is not evidence of what was built. Verify the complete assembled source before
and after the build/run and bind the actual executable hash to the build logs.
The narrow checker below does only identity verification and receipt writing; it
never builds, runs the compiler, compares semantics or adapts a source binding.

## Reproduction recipe

Run from the root of a trusted checkout containing this package and the two sibling authorities.
Use a fresh directory outside the repository. Keep all compiler sources,
requests, target files and outputs in separate paths. Commands below are a
manual recipe, not an automatically invoked CI entry point. Rust 1.99.0 and the
project's required build prerequisites must already be available. The retained
`rustc -vV` and `cargo -V` output is in `evidence`.

```bash
set -euo pipefail
REPO=$(pwd -P)
test "$(git -c safe.directory="$REPO" -C "$REPO" rev-parse --show-toplevel)" = "$REPO"
PACKAGE="$REPO/tests/fixtures/fixed_array_source_unit3/replay-v1"
AUTHORITY="$REPO/tests/fixtures/fixed_array_source_unit3"
TASK_RUN=$(mktemp -d)
export PACKAGE AUTHORITY TASK_RUN
mkdir "$TASK_RUN/source"
git -c safe.directory="$REPO" -C "$REPO" archive d582d1dca2a26eb3230632a921f9e8166c39d5c0 | tar -x -C "$TASK_RUN/source"
git -c safe.directory="$TASK_RUN/source" -C "$TASK_RUN/source" apply --check "$PACKAGE/inputs/checkpoint4-core.patch"
git -c safe.directory="$TASK_RUN/source" -C "$TASK_RUN/source" apply "$PACKAGE/inputs/checkpoint4-core.patch"
python3 -B "$PACKAGE/verify_inputs.py" --authority-root "$AUTHORITY" \
  --source "$TASK_RUN/source" --stage checkpoint4 --output "$TASK_RUN/checkpoint4-inputs.json"
cp "$PACKAGE/components/observer-v3/observer.rs" "$TASK_RUN/source/src/frontend/unit3a_independent_observer.rs"
printf '\n#[cfg(test)]\nmod unit3a_independent_observer;\n' >> "$TASK_RUN/source/src/frontend/mod.rs"
python3 -B "$PACKAGE/verify_inputs.py" --authority-root "$AUTHORITY" \
  --source "$TASK_RUN/source" --stage assembled --output "$TASK_RUN/assembled-inputs.json"
python3 -B "$PACKAGE/components/observer-v3/prepare.py" \
  "$AUTHORITY/contracts-v2" "$AUTHORITY/element-boundary-supplement-v1" "$TASK_RUN/requests"
rustc -vV > "$TASK_RUN/rustc.txt"
cargo -V > "$TASK_RUN/cargo.txt"
python3 -B - <<'PYTOOLS'
import json, os, pathlib
root = pathlib.Path(os.environ['TASK_RUN'])
fields = dict(line.split(': ', 1) for line in (root / 'rustc.txt').read_text().splitlines() if ': ' in line)
expected = {'release': '1.99.0', 'commit-hash': 'b940084d7eb6a299eb4bfeb8e34901bc051e7ac4',
            'host': 'x86_64-unknown-linux-gnu'}
if any(fields.get(key) != value for key, value in expected.items()):
    raise SystemExit('unsupported Rust toolchain; no build has started')
(root / 'toolchain-check.json').write_text(json.dumps(expected, indent=2) + '\n')
PYTOOLS
```

Build one profile at a time. The debug command uses the original debug-info
setting. This revised recipe adds `--offline` as a packaging choice for an
already populated dependency cache; the historical build did not specify it.
Missing cached dependencies cause a retained build failure rather than network
fetches. It also checks the full supported Rust commit and host before building
and writes the explicit build exit status. The exact-path Git trust settings
above apply only to those commands and change no global configuration.
No test executes during this build. This revised manual recipe has not been run
as part of the packaging checks.

```bash
export CARGO_TARGET_DIR="$TASK_RUN/target"
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
python3 -B - <<'PYBUILD'
import json, os, pathlib
root = pathlib.Path(os.environ['TASK_RUN'])
record = {'argv': ['cargo', 'test', '--manifest-path', str(root / 'source/Cargo.toml'),
                  '--bin', 'oxid', '--no-run', '--locked', '--offline', '--message-format=json'],
          'environment': {name: os.environ[name] for name in
                          ['CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'CARGO_INCREMENTAL', 'CARGO_PROFILE_DEV_DEBUG']}}
(root / 'build-debug-command.json').write_text(json.dumps(record, indent=2) + '\n')
PYBUILD
set +e
cargo test --manifest-path "$TASK_RUN/source/Cargo.toml" --bin oxid \
  --no-run --locked --offline --message-format=json > "$TASK_RUN/build-debug.jsonl" 2> "$TASK_RUN/build-debug.stderr"
BUILD_STATUS=$?
set -e
printf '%s\n' "$BUILD_STATUS" > "$TASK_RUN/build-debug.exit"
test "$BUILD_STATUS" -eq 0
python3 -B - <<'PY'
import json, os, pathlib
root = pathlib.Path(os.environ['TASK_RUN'])
rows = [json.loads(line) for line in (root / 'build-debug.jsonl').read_text().splitlines()]
paths = [row['executable'] for row in rows if row.get('reason') == 'compiler-artifact'
         and row.get('target', {}).get('name') == 'oxid'
         and row.get('profile', {}).get('test') is True and row.get('executable')]
assert len(paths) == 1, paths
(root / 'binary-debug.txt').write_text(paths[0] + '\n')
PY
BINARY=$(cat "$TASK_RUN/binary-debug.txt")
python3 -B "$PACKAGE/verify_inputs.py" --authority-root "$AUTHORITY" \
  --source "$TASK_RUN/source" --stage assembled --binary "$BINARY" \
  --requests "$TASK_RUN/requests" --output "$TASK_RUN/pre-debug.json"
mkdir "$TASK_RUN/debug"
"$BINARY" unit3a_ --nocapture --test-threads=1 > "$TASK_RUN/debug/focused.stdout" 2> "$TASK_RUN/debug/focused.stderr"
for SCOPE in core supplement observer-limits; do
  OXID_UNIT3A_REQUEST="$TASK_RUN/requests/$SCOPE.tsv" \
  OXID_UNIT3A_OUTPUT="$TASK_RUN/debug/$SCOPE" \
  "$BINARY" frontend::unit3a_independent_observer::unit3a_independent_observe_requests \
    --exact --ignored --nocapture --test-threads=1 \
    > "$TASK_RUN/debug/$SCOPE.stdout" 2> "$TASK_RUN/debug/$SCOPE.stderr"
done
python3 -B "$PACKAGE/verify_inputs.py" --authority-root "$AUTHORITY" \
  --source "$TASK_RUN/source" --stage assembled --binary "$BINARY" \
  --requests "$TASK_RUN/requests" --output "$TASK_RUN/post-debug.json"
cmp "$TASK_RUN/pre-debug.json" "$TASK_RUN/post-debug.json"
python3 -B "$PACKAGE/components/observer-v3/compare.py" "$AUTHORITY/contracts-v2" \
  "$TASK_RUN/debug/core" "$TASK_RUN/debug/core-comparison.json"
python3 -B "$PACKAGE/components/observer-v3/compare_supplement.py" "$AUTHORITY/element-boundary-supplement-v1" \
  "$TASK_RUN/debug/supplement" "$TASK_RUN/debug/supplement-comparison.json"
python3 -B "$PACKAGE/components/observer-v3/compare_limits.py" \
  "$TASK_RUN/debug/observer-limits" "$TASK_RUN/debug/limits-comparison.json"
PYTHONPATH="$PACKAGE/components/observer-v3" python3 -B "$PACKAGE/components/sensitivity-v5/sensitivity.py" \
  "$AUTHORITY/contracts-v2" "$TASK_RUN/debug/core" "$TASK_RUN/debug/sensitivity.json"
python3 -B "$PACKAGE/components/observer-v3/admission_controls.py" "$TASK_RUN/debug/artifact-admission.json"
python3 -B "$PACKAGE/verify_inputs.py" --authority-root "$AUTHORITY" \
  --source "$TASK_RUN/source" --stage assembled --binary "$BINARY" \
  --requests "$TASK_RUN/requests" --output "$TASK_RUN/post-debug-comparison.json"
cmp "$TASK_RUN/pre-debug.json" "$TASK_RUN/post-debug-comparison.json"
python3 -B - <<'PYLEDGER'
import hashlib, json, os, pathlib
root = pathlib.Path(os.environ['TASK_RUN'])
files = list((root / 'debug').rglob('*'))
files += [root / name for name in ['rustc.txt', 'cargo.txt', 'build-debug-command.json',
          'build-debug.jsonl', 'build-debug.stderr', 'build-debug.exit', 'toolchain-check.json', 'binary-debug.txt',
          'pre-debug.json', 'post-debug.json', 'post-debug-comparison.json']]
entries = {str(path.relative_to(root)): {'bytes': path.stat().st_size,
           'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
           for path in sorted(files) if path.is_file()}
with (root / 'debug-artifact-manifest.json').open('x') as output:
    json.dump({'schema': 'oxid-unit3a-replay-artifacts-v1', 'files': entries}, output, indent=2)
    output.write('\n')
PYLEDGER
```

For release, run the same build command with `--release`, write separate
`build-release` logs, select its executable from that build's JSON, and repeat
the pre/post checks, focused tests and three observation scopes under a fresh
`release` output directory. Use separate receipt/report names throughout.
Compare the exact artifact rosters and bytes for all 58+6+4 outputs between
profiles; report any difference. Do not overwrite a failed attempt or reuse its
output directory. Retain the first failure and its exact source/component/input
and binary bindings before diagnosing or changing any implementation.

The exact frozen authorities are rehashed before and after observations by the
checker, including every listed body. Relocated TSV hashes will differ from the
historical path-dependent request hashes, so each fresh TSV receives a new
receipt. The v2 packaging checker independently derives the exact ordered
58 core, six supplemental and four lowered-limit tuples from the frozen
authorities and fixed historical control definition. Labels, single/project
modes, canonical absolute relocated source paths and both exact limits must
match. Missing, extra, duplicate, reordered or otherwise altered rows/fields
are rejected before an identity receipt is written. This stricter checker
changes no executed observer, request generator or comparator bytes. The
four observer limits are transport controls, never source-language results.
Source allocation identities are positive/distinct within each observation;
structural origins and child IDs come from actual APIs, not reconstructed text.

The checker has a separate bounded negative-control command. It prepares inert
request data in two fresh locations, accepts both canonical rosters, then rejects
48 mutations across all three TSVs. It preserves every mutated TSV, checker
stdout/stderr and command/result record, and runs neither the compiler nor the
semantic comparators:

```bash
python3 -B "$PACKAGE/test_request_identity.py" --authority-root "$AUTHORITY" \
  --output "$TASK_RUN/request-identity-controls"
```

The v1 packaging checker only hashed supplied TSVs and accepted five independently
reported changes: empty core input, unknown label, altered mode, altered row
limit and a nonexistent source path. Its original envelope and failure records
remain retained separately. The v2 strict-tuple check addresses that packaging
gap; it does not alter the previously executed source qualification.

## Adapting to a future current compiler

A current checkout may include additional source or tooling changes. It does not
satisfy the historical assembly merely because the observer builds. Do not
replace the frozen manifests, alter expected outputs from candidate results or
remove the source-binding comparison. A future adaptation must explicitly bind
its new compiler file closure, observer and comparator source-header identity,
build tools, binary, request roster, authority manifests and output completion
rules, and independently qualify the changed scope. This package does not claim
that adaptation, PR28 prerequisite integration, normal hosted CI or activation.
