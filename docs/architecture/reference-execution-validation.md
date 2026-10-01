# Bounded bool/unit reference execution validation

Date: 2026-10-01. Scope: explicit `run --edition typed-preview` from a zero-argument
main, through complete source checking and mandatory OIR verification, followed
by isolated iterative scalar activations with deterministic limits. This remains
experimental and provisional; [RFC 0004](../../rfcs/0004-bounded-reference-execution.md)
defines the contract. No native compilation, numeric semantics, ownership,
memory-safety, OS-sandbox, stable edition, or completed M2/M3 claim follows.

The layout measurements below describe the bool/unit-only predecessor snapshot.
The subsequent [i32 literal increment](i32-literal-validation.md) increases scalar
storage and records current measurements; do not reuse this report's one-byte
slot estimate for that extended representation.

## Snapshot and host

- Base: `e998a8d5b1a680e9e2d723352a45a547d3935114`
- Base tree: `458db76c404c068d6f5bafb752b02356255058a0`
- Started on the exact predecessor head `cbaa8734aef68dc23fb4bc4c8ce09c71123a5308`;
  reconciling to its identical-tree merged base preserved the working diff bytes
- Snapshot: base plus this increment; publication commit and remote CI are not
  claimed by this local report
- Code/test manifest SHA-256: `4a896574edc72aaae37b74976d22067109150a820ae783153bd42df21627243c`
- The manifest maps all `src/**/*.rs` and `tests/*.rs` relative paths, sorted,
  to SHA-256 hashes, encoded as Python `json.dumps(map, indent=2) + "\n"`.
  Documentation is outside the code/test digest
- Host: Linux x86_64, `x86_64-unknown-linux-gnu`
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`; Python 3.12.14
- No Cargo/dependency, grammar, resolver, type-checker, lowering/verifier algorithm,
  raw IR shape, legacy runtime/artifact, default-edition or README layout changes

## Verification record

All commands below were run successfully on the integrated candidate after the
last code/test edits. These are local results, not remote CI results.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | Formatting passes |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | 0 | Included legacy syntax checked explicitly |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | No warnings |
| `cargo test --all-targets --all-features --locked` | 0 | 238 Rust tests pass |
| `cargo build --release --locked` | 0 | Optimized standalone binary builds |
| `cargo test --release --bin oxid frontend::oir --locked` | 0 | 94 OIR tests pass |
| `cargo test --release --test typed_execution --locked` | 0 | All 8 public run tests pass optimized |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 0 | 7 metadata tests pass |
| `python3 scripts/verify_repo.py target/release/oxid` | 0 | 121 sources, 67 runnable programs, 17 feature entries |
| `git diff --check` | 0 | No whitespace errors |

Debug counts: 182 unit tests, 12 edition-boundary tests, 1 generated-document
test, 8 legacy-module tests, 8 legacy-semantic tests, 8 typed-execution tests and
19 typed-frontend tests. The 214 predecessor tests remain, with 24 added.
The prior typed-run rejection fixtures now retain exact unsupported-source
rejection and use compile for still-unsupported command rejection. No source
fixture was added to the repository-wide legacy `.ox` discovery set.

## Requirement-to-evidence map

| Contract | Tests |
| --- | --- |
| Explicit run only, global/command error-summary distinction, source read/UTF-8/artifact failures | `typed_execution` public CLI tests, retained edition-boundary suite |
| Exact true/false/unit stdout and schema-1 run-summary; false exits 0 | Public CLI repeated scalar cases |
| Whole-file verification before entry/execution; main resolved by compiler identity | Invalid unchosen branch, missing main, main arguments, invalid private ID, arbitrary valid raw-origin tests |
| Calls, isolation, branches, missing else, continuations and unit copies | Source integration fixtures, finite direct/mutual recursion and independent model |
| Argument order, single condition evaluation, discarded call execution, unchosen divergence | Exact divergent-left-helper origin, trace comparison and independently counted 15-fuel source |
| Charge-before-work and fixed error precedence | Raw 4-fuel constant, every instruction-kind, zero/one-short, call/frame/slot preflights |
| Live storage and independent caps | Returned-slot reuse, exact 1,024 frames, exact 200,000 slots and plus-one-local case, 256 arguments |
| Repeated fresh runs and nontermination | Repeated immutable witness/CLI invocations; deep mutual recursion and exponential shallow call graph |
| No raw unverified execution boundary or unsafe diagnostic rendering | Verifier rejection, test-only forged witness faults, invalid-origin E0500 |
| Legacy compatibility | Full original suites, unchanged check summary, repository verifier and no new discovered `.ox` fixture |

## Independent oracle and resource interpretation

A separate test-only structured-source interpreter computes expected scalar
results and ordered enter/branch/return traces before the source is printed and
passed through the real lexer/parser/resolver/type checker/lowerer/verifier/runner.
It uses name environments and block evaluation, not OIR slots, production
dispatch/lowering helpers or the legacy VM. Its acyclic call graphs and bounded
nesting permit a small independently structured recursive model. All 16 boolean
two-input truth tables are run for all four inputs (64 fixtures), plus four
unit/mixed-return/scope/group/discard fixtures. Independent bit-table assertions
also check the generated oracle's truth-table results. Test-only observations
are absent from production builds and are not a public tracing API.

Pure result equality alone cannot reveal duplicated/reordered calls. Trace
comparisons, discarded divergence, unchosen divergence and exact hand-counted
costs therefore complement value comparisons. Source `if id(true)` selecting
`return false` costs 15: root 5 + true 1 + call 4 + id copy/return 2 + branch 1 +
selected false/return 2. Budget 14 fails at the selected return. The raw one-local
constant/return fixture costs exactly 4. A 256-argument source fixture costs
1,031 and uses exactly 514 live slots. Counts are written independently of
production accounting helpers.

On this host the measured scalar and optional-slot sizes are each 1 byte,
Frame is 96 bytes and Resume is 40 bytes. The maximum frame-header vector
capacity is 98,304 bytes; live scalar storage is at most 200,000 bytes; transient
argument scratch is at most 256 bytes. This is about 298,560 bytes plus vector
headers, counters and allocator overhead, excluding compiled IR, source,
compiler/verifier scratch and test observations. No frame-times-largest-function
preallocation occurs. Host allocation can still fail. Fuel bounds charged
abstract-machine work, not end-to-end elapsed time, allocation success, source
read latency or output-pipe behavior. The runner is not an OS sandbox.

## Development evidence and limits

The initial six public CLI tests were observed red against the old run rejection,
then green after the implementation. Resource boundary tests were added after
that initial implementation, rather than being misrepresented as individually
red-first. The independent model trace tests were observed red with missing
observations, then the observation events were connected. Initial test-harness
compile mistakes, a hand-counted two-versus-three-slot expectation and an obsolete
well-formed run-rejection fixture were corrected without changing production
semantics. That fixture now tests still-unsupported compile; new run tests
cover the intended changed boundary. Final results appear above.

Local results do not establish remote CI, Windows/macOS/Linux ARM64 behavior,
ASan/TSan, hard real-time behavior or a general sandbox/security proof. Existing
CI will exercise supported configured hosts separately after publication. The
whole roadmap remains open beyond this bounded increment.
