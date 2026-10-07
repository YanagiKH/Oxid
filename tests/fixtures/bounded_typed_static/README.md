# Bounded two-process scalar static qualification

`scripts/verify_bounded_typed_static.py` checks the actual `parser_main.ox` and
`ast_static_main.ox` roots, then compares complete typed facts or the first
diagnostic for all 81 retained sources. `--native` also compiles both roots with
the normal LLVM command and executes the same matrix through both ELF files.
The host only frames successful parser bytes as `AST1 + source length + exact
source + exact OPA1`, or relays an unchanged parser failure. The existing
`static_typed_observation.py` projection supplies no semantic inference.

Build the independent observers in fresh directories, then run on Linux:

```sh
python3 scripts/build_typed_lexer_observer.py --output /tmp/static-lexer-observer
python3 scripts/build_typed_static_observer.py --output /tmp/static-observer
python3 scripts/verify_bounded_typed_static.py \
  --oxid /absolute/path/to/qualified/oxid \
  --lexer-observer /tmp/static-lexer-observer/canonical-lexer-observer \
  --static-observer /tmp/static-observer/canonical-static-observer \
  --output /tmp/bounded-static-evidence --native
```

Omit `--native` for actual reference execution only. Linux and `cc` are required
for the retained process-I/O controls. Output must be a fresh directory.
Compiler qualification belongs to the caller's build/provenance
workflow; this controller records its actual hash and refuses changes during
the run. Observer build receipts, binaries, and copied source files must match
the unchanged sources and wrapper selection of the existing builders.

## Fixed matrix and explicit refusal

`cases.json` is a byte-for-byte copy of the reviewed
`ast-semantic-entry/authored-cases.json`, SHA-256
`985c6f5286b86591bdb5945ef13df66bce7b5bbdd2ff4cdb8e8a4cd7742a2df3`.
Its 77 syntax-success cases exercise the consumer; four named cases relay the
parser's lexical, syntax, or domain refusal. Each selected mode must finish all
81, with no pending entries, duplicates, or silent reclassification. Native
qualification additionally requires exact parser and final-output wire parity
for every source.

`canonical-039` retains the exact source `struct S{x:i32}`. The canonical
observer's generic record refusal covers bytes `0..15`; the existing parser's
explicit record-keyword refusal covers bytes `0..6`. Both are checked against
their own exact contract. This case is not canonical static diagnostic parity,
and no other unknown or outside-subset case is normalized.

## Evidence and limits

The source snapshot contains only the fixed 30-module closure of the two roots
and the unchanged host projections. `ast_consumer_probe.ox` remains a separate
closed validation root. Each native execution has a cleared environment and an
empty working directory while the copied source directory is renamed. Injected
I/O runs add only their recorded preload and fd1-target overrides. This
demonstrates execution independent of those copied source paths, not a filesystem
sandbox. Check/compile commands, exact inputs, stdout, stderr, remaining stdin,
hashes, and a progressively saved failure-first report are retained.

`malformed_controls.py` stores exact prior independent AST1 inputs compactly,
with input hashes and provenance. Its byte packaging performs no semantic work.
The controller selects 22 existing malformed controls from the original 59
closed-boundary checks, seven from the earlier focused validation controls,
and the previously reviewed final-byte truncation and two trailing witnesses.
All 31 run through the real semantic consumer in each
selected mode and require status 64, empty output, and exact observed input
consumption. The trailing-witness input must consume `A` and leave `B` unread.
Real directory-input and closed-pipe controls require status 74. Closed pipes
exercise both inherited SIGPIPE policies for success and diagnostic inputs,
delivering zero bytes.

The complete existing I/O selection is 48 executions per mode: two ordinary
transport baselines, 31 malformed inputs, one real input failure, four real
closed-pipe output failures, and ten injected-prefix failures. `--native`
requires all 96 executions and exact status/stdout/stderr/remaining-input parity
for all 48 pairs. The ordinary baselines supply only transport-prefix oracles,
not independent semantic expectations.

`stdout_prefix_eio.c` and `calibrate_stdout_prefix.c` are byte-identical copies
of the separately reviewed I/O evidence sources. Their exact hashes are pinned
before compilation. Calibration proves a seven-byte fd1 write returns three,
the next fd1 write gets EIO, and stderr plus an unrelated descriptor are
unaffected. Only after that calibration succeeds does the controller inject
the same five prefix cutoffs per baseline: 0, 1, 1559, 1563, and the last output
byte. These are calibrated injected short-write/EIO controls, not real-kernel
partial-write or partial-output SIGPIPE evidence. Every source, build command,
compiled support hash, override, and actual prefix is retained. The original
96-execution report remains separate historical evidence.

The original failed complete replay remains historical evidence. The corrected
81-case replay and separate original malformed/I/O evidence are not overwritten
or replaced by controller results.

`python3 -m unittest discover -s scripts -p test_bounded_typed_static.py` checks
controller validation using fake processes. Those tests are portable validation
logic checks, not reference, multi-file-loader, native, or kernel-I/O evidence.
Actual qualification runs separately in the Linux LLVM job.
