# Unit4 two-group scheduling v1

Review base: `31f49568b1142dd58d80eef8f71faeeb99f32ba5`, tree
`c31f377ccca7ea34b8f741e8ce725975108e481d`.

Unittest module discovery defines the closed 174-ID roster in 23 classes,
including 37 tests in three imported/inherited identity classes and three new
parser budget refusal controls. Each host has exactly two fixed groups:

- comparison-seal: all 32 ComparisonSealControls methods, one serial class suite.
- remaining-controls: all other 22 complete classes, 142 IDs, serially executed.

Four hosts (Linux x86_64, Windows x86_64, macOS x86_64, macOS arm64) therefore
require eight integration jobs. Each group has a fresh checkout/process; no
mutable fixtures are shared across groups. The groups are disjoint and their
union is exactly actual discovery, checked against the pinned roster.

## Finite scheduling ceilings

Run 38830442781 timestamped test logs show ComparisonSealControls spanning
33.97 minutes on Linux, 36.54 on macOS arm64 and 86.77 on macOS x86_64 between
its first and last test lines. Windows reached only 27 passing methods and an
interrupted 28th after 92.74 minutes between its first and last class lines.
These are inter-line wall-clock spans, not exact isolated per-method execution
measurements; they omit the first interval and may include buffering/setup.
Linux/macOS-arm64 full 171-case suites completed in approximately 44/47 minutes;
the cancelled x86 hosts do not provide complete remaining-group measurements.

A 100-minute comparison job has insufficient demonstrated headroom on Windows.
Use a finite 150-minute ceiling for comparison-seal and retain 100 minutes for
remaining-controls. The extra comparison margin covers the unfinished tail,
setup/upload and runner variability; it is an estimate requiring hosted replay,
not a claim of a successful complete Windows run. No evidence currently
justifies a third group. The original evidence is [run 38830442781](https://github.com/YanagiKH/Oxid/actions/runs/38830442781):
[Linux job 116747761549](https://github.com/YanagiKH/Oxid/actions/runs/38830442781/job/116747761549),
[macOS arm64 job 116747761814](https://github.com/YanagiKH/Oxid/actions/runs/38830442781/job/116747761814),
[macOS x86_64 job 116747761833](https://github.com/YanagiKH/Oxid/actions/runs/38830442781/job/116747761833), and
[Windows job 116747761861](https://github.com/YanagiKH/Oxid/actions/runs/38830442781/job/116747761861).

## Admission and evidence

The explicit integration join requires all eight artifacts and successful matrix
status, exact group/test IDs and started/pass lists, measured host and
head/tree/event/run/attempt/roster identity. Missing, duplicate, unknown, failed,
skipped, expected-failure, cancelled or stale results fail. Artifact names
include run/attempt and are checked against receipts; earlier attempts remain
preserved and cannot fill missing groups. After class teardown each shard repeats full admission and compares its starting
identity before recording success. Output paths reject symlink ancestors; receipt
and approval inputs reject symlinks and hardlinks. Each shard uploads its full unittest
log and receipt with always(); admission errors preserve tracebacks. Hard
termination can prevent upload: missing evidence fails, with GitHub job logs
remaining the infrastructure diagnostic source.

Both formal qualification jobs need the integration join, then download and
verify its approval for the current run/attempt before entering the gate. This
also rejects rerun-failed reuse of successful dependencies from older attempts.
The final join's exact two upstream keys and original capsule handling remain.

## Separate measured prepare budget amendment

The named gate_scheduling.py entrypoint pins and loads the byte-identical
accepted gate.py, keeping the historical accepted-view dependency valid.
Only 08-parser-prepare changes from 300 to 600 seconds, based on the separately
reported unoptimized complete prepare observations 396.72/416.59 seconds.
Passivity 300 seconds, combined 9000-second deadline, compiler fuel and all
source/identity/ABA checks remain unchanged. New controls test the actual
watchdog kill and full failure receipt, timed_out with status zero rejection,
and the minimum of stage allowance and remaining combined deadline. The real
watchdog test waits for a bounded child-ready handshake (30-second startup cap)
before its two-second runtime deadline, avoiding slow-host startup flakiness.

## Mechanical activation

The named derivation pins the predecessor workflow and roster; ci.py and
 derive_consumers.py use the same bounded entry point. All 173 original consumer
obligations remain; only L62/L988/L1071 active controls placements move, and L997 selects the
named Linux prepare-budget entrypoint. Existing
source-argument derivation runs first. Historical workflow bytes, disposition,
consumer proof, input roster, inputs-package-json-v1.json, source manifests and
checkpoints are retained. A named input generation updates only three approved
current dependencies and adds the new disposition; all other identities must
remain exact. The new gate entrypoint and integration-test amendment are handwritten; the
accepted gate.py bytes remain unchanged.

Full repository verification fetches only the fixed predecessor with `--depth=1`
and checks its exact tree before scheduling controls. Other jobs do not fetch history.
For a shallow checkout, first run `python -B tests/qualification/unit4_ci/derive_scheduling.py --fetch-base`.
At the specified base, apply the small patch and run:

```
python -B tests/qualification/unit4_ci/derive_scheduling.py
```

Before the tests below, create an isolated local commit containing the generated
candidate and all added files. The checkout regression fixtures require those
files in the committed snapshot; staging new files alone is insufficient.

```
python -B tests/qualification/unit4_ci/test_scheduling.py
python -B scripts/test_lexer_current_ci.py
python -B -m unittest discover -s tests/qualification/unit4_ci -p test_integration.py -k LexerInputClosureControls -v
python -B -m unittest discover -s tests/qualification/unit4_ci -p test_integration.py -k parser_timeout -k parser_budget -k shared_preparation -v
```

The derivation emits seven files including the discovery roster. Do not invoke
old activate_ci.py/activate_consumers.py; those are historical activation flows.
Production admission requires committed bytes. Local tests and a successful
prepare alone are not full four-host execution qualification.
