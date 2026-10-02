# Typed-project Unit 1: reproducible single-file compatibility

Run the exact frozen 35-case corpus against existing debug and release CLIs:

```sh
python3 -B tests/fixtures/typed_project_unit1_compatibility/run.py \
  --debug target/debug/oxid \
  --release target/release/oxid
```

Python 3.11 or newer and the two existing CLI binaries are sufficient. No build,
network, dependency install or repository write occurs. The single entrypoint
materializes the byte-identical generator and CLI engine in a fresh retained
system temporary directory, runs them with bytecode generation disabled, and
prints that evidence path. Fixtures retain their original relative
`fixtures/<case>.ox` argv and diagnostic display paths. There are no installed
`.ox` files to enter the legacy repository scan.

Use `--evidence-root /absolute/existing/external/directory` to select the parent
of the fresh temporary directory. It must be outside the package and detected
input repositories. Generated sources, helper files, streams and receipts all
remain under the new temporary directory. Neither installed helper bytecode nor
temporary evidence is written into the package. Evidence is retained on success
and failure; remove the printed temporary directory later when no longer needed.

The full matrix is 35 cases × check/run × JSON/text × debug/release × two repeats:
**560 invocations**. The runner requires every expected key exactly once, all
1,120 stream artifacts, exact predecessor exit/stdout/stderr bytes, independent
semantic expectations and repeated/profile equality. Zero, partial, duplicate,
extra or mismatched observations cannot pass. Binary, fixture and package
identities are checked before and after execution. No source/output/path
normalization is applied.

`--original-only` is reserved for a future deliberate activation that tests the
31 OriginalSingleFile cases separately: 496 invocations. **Do not use it for the
Unit 1 gate.** The four separately labeled mod/use/pub/qualified-path cases must
retain their old diagnostics while project syntax remains disabled. This flag
does not accept or record new expected migration outputs.

## Package and provenance

- `generate_fixtures.py`: the frozen source generator, copied byte-for-byte
- `suite-intent.json`: the unchanged independent source/semantic contract
- `cli_engine.py`: the frozen corrected 560-invocation engine, copied byte-for-byte
- `expected-results.json`: 140 canonical predecessor results, one per
  case/operation/format; exact stdout/stderr bytes are base64 encoded
- `run.py`: temporary materialization, orchestration and a completeness/byte audit
- `package-manifest.json`: exact package hashes and predecessor provenance

Canonical records use the predecessor debug/repeat-1 result. During packaging,
each was compared to all four original profile/repeat observations and all raw
streams were rehashed. The source generator, intent, expected values and cases
were not edited or regenerated from candidate output. Each canonical record
contains the original full baseline row's SHA-256 (canonical JSON with sorted
keys and separators `,` and `:`), and the file records the full original baseline
manifest SHA-256:

`0c359ecf1456ddb1996fc6aa7c28289e8b72262a0305b7b317c678f7f93dcec0`

The exact predecessor is commit
`fbcfeb2a2de8fe9d335d6c8051d254cccdb663dc`, tree
`658c83465aed3a1423483dcdcc23a3d10037131f`.
The baseline's original post-manifest `int(None)` exception and all historical
receipts remain separately preserved; the copied engine is the already-frozen
corrected version. Packaging neither hides that exception nor counts historical
observations as new runs.

For the copied engine's existing comparison interface, the entrypoint expands
canonical hashes into a temporary comparison-key table. That table is explicitly
labeled derived expectations with zero executed observations. It does not store
another canonical stdout/stderr set or claim another execution. Actual compiler
observations are only in `evidence/matrix/manifest.json` and its raw streams.

The top-level `receipt.json` records process statuses, commands, package/binary/
fixture hashes and all retained stream hashes. `audit.json` records audited
counts. `engine.stdout` and `engine.stderr` preserve the matrix process logs.
The package gate confirms focused public compatibility; it does not replace
private source-map, path, resource or scalar-HIR gates, native qualification,
complete fuel proofs or publication review.

## Narrow independent check of one retained full run

The following uses only stdlib JSON/base64/hash/filesystem operations. It imports
no package code and runs no compiler. Set the first path to this package and the
second to the printed temporary evidence directory:

```sh
python3 -B - tests/fixtures/typed_project_unit1_compatibility /tmp/oxid-unit1-compatibility-EXAMPLE <<'PY'
import base64, hashlib, itertools, json, sys
from pathlib import Path
p, e = map(Path, sys.argv[1:])
load = lambda f: json.loads(f.read_text())
sha = lambda f: hashlib.sha256(f.read_bytes()).hexdigest()
pkg = load(p / 'package-manifest.json')
for name, info in pkg['files'].items():
    assert sha(p / name) == info['sha256']
intent = load(p / 'suite-intent.json')
cases = {c['name']: c for c in intent['cases']}
assert len(cases) == 35
assert (e / 'suite-intent.json').read_bytes() == (p / 'suite-intent.json').read_bytes()
for c in cases.values():
    assert sha(e / c['path']) == c['sha256']
canonical = load(p / 'expected-results.json')['canonical_results']
expected = {(r['case'], r['operation'], r['format']):
            (r['exit_code'], base64.b64decode(r['stdout_base64']), base64.b64decode(r['stderr_base64']))
            for r in canonical}
assert len(canonical) == len(expected) == 140
receipt = load(e / 'receipt.json')
assert receipt['status'] == 'PASS' and receipt['engine_exit'] == 0
assert receipt['binary_hashes_before'] == receipt['binary_hashes_after']
assert receipt['fixture_hashes_before'] == receipt['fixture_hashes_after']
assert receipt['package_hashes_before'] == receipt['package_hashes_after']
for profile, binary in receipt['binaries'].items():
    assert sha(Path(binary)) == receipt['binary_hashes_before'][profile]
out = e / 'evidence/matrix'
rows = load(out / 'manifest.json')['rows']
key = lambda r: (r['case'], r['profile'], r['operation'], r['format'], r['repeat'])
wanted = set(itertools.product(cases, ('debug', 'release'), ('check', 'run'), ('json', 'text'), (1, 2)))
assert len(rows) == len(wanted) == 560 and {key(r) for r in rows} == wanted
artifacts = set()
for r in rows:
    c = cases[r['case']]
    assert r['source_sha256'] == c['sha256']
    assert r['argv'][1:] == [r['operation'], '--edition', 'typed-preview', '--message-format', r['format'], c['path']]
    streams = []
    for stream in ('stdout', 'stderr'):
        filename = r[stream + '_file']
        assert filename not in artifacts
        artifacts.add(filename)
        assert sha(out / filename) == r[stream + '_sha256']
        streams.append((out / filename).read_bytes())
    assert (r['exit_code'], *streams) == expected[(r['case'], r['operation'], r['format'])]
actual = {f.name for f in out.glob('*.stdout')} | {f.name for f in out.glob('*.stderr')}
assert len(artifacts) == 1120 and artifacts == actual
print('PASS: 560 exact canonical comparisons; 1120 stream files; fixed source/binary/package identities')
PY
```

When the full original baseline is available for review, its manifest hash and
each selected debug/repeat-1 row hash can additionally be recomputed against
`expected-results.json`. That integrity check is historical provenance, not a
new compiler run.
