# Independent typed-project Unit1 regressions

This package preserves the independent source-set/loader regression corpus that
qualified the v4 core in69 test functions per profile. It is suitable for a
repository directory such as `tests/fixtures/typed_project_unit1_independent/`.
It does not activate project source syntax or edit the supplied repository.
Output, Cargo cache and TMPDIR must be outside that repository; optimized Python
(-O or PYTHONOPTIMIZE) is refused because it can remove qualification guards.
The entry point disables helper bytecode caches before importing local modules,
so running an installed copy creates no __pycache__ in the repository.

Run from a machine with the repository's Rust toolchain and cached dependencies:

```sh
python3 tests/fixtures/typed_project_unit1_independent/run.py \
  --repo . --profile both --output ../oxid-unit1-regression-results
```

`--profile debug` or `--profile release` runs one profile. `--cargo` can select a
Cargo executable; standard `PATH`, `RUSTC` and `CARGO_HOME` settings are inherited.
Builds use `--offline`. `--target-dir` optionally points to an isolated reusable
Cargo cache. The output directory must be new or empty; omit it for a unique
results directory. Only Python's standard library is required by the runner.

The runner requires Linux x86_64. Unsupported hosts produce a skipped result and
exit77. This is frontend/filesystem qualification, not a new native target.

## Execution and retained evidence

For every run, the runner creates a unique temporary source copy and fixture
root, verifies the independent source/count/diagnostic expectations, and injects
the test module only into that copied source. The fixed source and display paths
are relocated to this per-run root. No checked source or expected diagnostic
value is obtained from compiler output.

For each requested profile it builds the exact test binary, discovers exactly69
independent tests, executes them individually with a25-second per-test timeout,
requires an exact frozen69-name roster and precisely one passing, nonignored
execution of each named test, and saves stdout/stderr and hashes. Test listing is
also timed out and retained. Zero-case or failed standalone runs exit nonzero. It verifies the source inventories and full
directory names/counts immediately before and after each profile. It checks that
the supplied source, compiled source and binary remain unchanged. The temporary
source and fixture trees are removed afterward; result manifests, generated
review tests, input hashes, toolchain output and individual logs remain in the
output directory. Failures remain failures; there is no retry-until-green loop.

`result.json` is the top-level result. `debug-results.json` and
`release-results.json` contain actual test-function counts and binary hashes.
Do not count historical v1/v3 runs as new evidence. The original final v4 core
had69/69 pass in both profiles, totaling138 actual test-function executions.
Packaging validation is a separate run on its own recorded source/binary hashes.

## What the count means

The69 functions include53 independent filesystem/default-resource fixture tests,
origin/handle/frozen-source checks, lowered/preflight checks, measured layouts,
production scan-iterator controls and one direct scalar-HIR compatibility probe.
Several functions contain multiple independently specified scenarios:

-16 controlled reserve failures per profile, via actual fallible reserve errors
-16 checked-counter overflow scenarios per profile through real adapters
-4 direct vector-length/product, String and PathBuf arithmetic controls
-8 checked ChildPlan overflow cases
-7 malformed origin spans and independent three-file diagnostic labels

The scalar-HIR probe came from the separate compatibility reviewer. It is private
seam evidence, not part of the560 public CLI observations. Its source hash and
all expectation provenance are in `expectation-provenance.json`.

The42 authored filesystem cases include one unavailable Unix-socket subtype.
The generator explicitly records its prior environment denial and never tries
to create a socket. The41 representable cases include FIFO, directory, symlink,
hardlink, exact-case and native-byte identity controls. Twelve additional resource
fixtures exercise real default byte/token/module/depth/E/U exact/over boundaries.
The exact expected fixture inventories are retained alongside the generators.

## Scope and limits

The tests qualify private Unit1 loading and immutable source identity. Public
mod/import/pub/qualified-path activation, final global DefId/RecordId semantics,
shared index I/J/W admission and universal index redundancy, visibility/import
semantics, linked execution and native project qualification remain later gates.

V8448 is masked by M256/D32 (maximum7664 probes); its check uses lowered seams.
P and relative/component path mechanics use reduced seams where host pathname
limits mask or complicate the default boundary. No total RSS, allocator-capacity,
total OOM recovery, TOCTOU sandbox or OS-I/O blocking guarantee is claimed.
