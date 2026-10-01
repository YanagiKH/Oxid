# Ordinary while and shared fuel: local validation

Scope: [RFC 0012](../../rfcs/0012-while-runtime-fuel.md), based on merged main
`652925442c2b3f1602c55ecbbe1b1f9d08d34de1` (tree
`0a7ac1c2ba75d6bba4ca556b498bdc71e072e02e`). Implementation began on published
PR15's identical tree and reconciled without altering working source bytes.
This is local evidence for an experimental preview, not a hosted-CI result for
this candidate, stable ABI, ownership certification or completed roadmap milestone.

## Environment and restoration boundary

2026-10-01, Debian 13 Linux x86_64/glibc 2.41, Rust/Cargo 1.98.1 and Debian
LLVM/Clang/LLD 19.1.7. Both compiler profiles emit native O0; O2/LTO and other native
hosts are unqualified. During setup the execution workspace was replaced. The
published PR15 tree was recloned and verified exactly; official checksum-verified
Rust and signed Debian package-index-selected LLVM packages were restored. A
fresh clean published-base suite passed 359 ordinary tests with two explicit LLVM
ignores. No claim depends on unavailable pre-reset local logs.

Production changes cover while parsing/resolution/flow/lowering, cyclic dominance,
repeat execution of verified scalar definitions, shared native fuel and guarded
representation limits. The C error/output adapter, tool trust, no-clobber artifact
publication and legacy/OXBC paths remain unchanged. The human renderer now also
streams its identical bytes to a counting writer. Old unsupported-while controls
become still-unsupported for/loop/match controls; accepted while has dedicated
coverage. Old raw Cycle expectations are replaced by valid-cycle or independently
unchanged unreachable/definition errors, not removed without replacement.

## Focused and independent proof checks

The new public suite first failed with E0101 on while. The verifier specialist's
raw legal cycle first failed with the intended old Cycle rejection. Subsequent
focused/reference/native suites pass; not every later adversarial fixture had a
separately demonstrated predecessor failure.

Six public groups cover zero/one/many/nested iterations, bool/i32/unit places,
per-iteration initialization, snapshots, calls, lazy conditions, returns, lexical
reuse/no-shadowing, conservative all-path returns, dead-body checking, unavailable
break/continue/while-expressions, and inclusive64 expression/block depth ceilings.
A native CLI group checks valid while reaches missing-tool E0701 only after
admission, and invalid conditions/dead bodies/recursive call graphs fail before
artifact/tool effects.

Independent cyclic verifier evidence:

- All 4,829 reachable simple directed graphs through four vertices with at most
  two successors per vertex, including self-edges, cycles and irreducible graphs
- 303,561 path-removal dominance comparisons with every raw entry position
- 5,846 whole-IR value/call/place availability cases and 2,056 merge-edge cases
- 1,048,576 additional comparisons over 128 larger cyclic graphs in both successor
  orders; total direct dominance comparisons 1,352,137
- Exact 300,000-block deep cycle and 60,001-block irreducible ladder
- Static unique definitions/canonical initialization, first-visit ordering,
  same-block forward reads, normal call edges, self/backedge merges, duplicate
  edges, invalid identities and unreachable blocks

These are exhaustive only over the stated small graph family, not all CFG sizes.
The O((B+E)log B) work/O(B+E) space argument comes from the implemented iterative
simple Lengauer–Tarjan invariants and original algorithm, not extrapolated timing.

Reference tests hand-count a nine-slot while counter at 46 fuel and assert the
exact failure span for every lower budget. A one-frame/nine-slot limit still
executes all iterations, proving no per-iteration allocation growth; eight slots
fail. Unit-only events show fresh body initialization, persistent outer state,
and no Store after failed fuel/overflow. Static parameters remain immutable.

The raw cyclic execution suite hand-counts 13/23-unit completion and 15/16-unit
fuel-before-overflow cutoffs. Its mandatory real-LLVM test passed in both compiler
profiles, producing 116 executables and rejecting eight deliberately invalid phi
modules. It covers prior-iteration/self-input bool merges, direct call-result
edges, self-continuation calls, irreducible mutable flow, checked arithmetic/call
predecessor splits, guard labels and reversed/nonzero entries. The two predecessor
raw tests also passed in each profile:40 executables and four invalid phi modules.
Thus all three raw gates executed 156 artifacts and twelve negative LLVM modules.
Ordinary suites report their three ignores separately rather than counting them
as executed there.

## Native representation bounds

Both guarded-only default caps are exercised against verified 100,000-instruction
raw cyclic modules: 16MiB human diagnostic data and 64 MiB LLVM text. Exact formatter
sizes are admitted and a one-byte smaller allowance is rejected; checked counters
also test each default maximum and the next byte. Acyclic module admission ignores
these new guarded-only limits. An initial raw stress fixture remained below 64 MiB;
its path length was increased before claiming default-cap rejection. This was a
test-size correction, not a changed product limit.

Diagnostic output is streamed through the shared human renderer before a message
allocation, and LLVM bytes are counted before module allocation. Tests include
escaped newline/tab/Unicode paths, deduplicated failure-kind/origin keys, unused
cyclic functions forcing guards in loop-free main, preserved static rejection of
expensive acyclic functions, and exact final predecessor labels.

## Independent source-state/fuel model

The Python oracle owns source ASTs, tagged scalars, lexical cells, call activations
and an execution schedule derived from the specification. It reuses the earlier
independent scalar rendering primitives, not compiler IR or compiler-generated
expectations. Arbitrary-precision arithmetic checks each executed result; offsets
are calculated from rendered source, including a Unicode/CRLF prefix. Self-checks
establish the grouped counter at 55 fuel and failure at every lower budget.

The completed modeled corpus passed both profiles:

- 144 source programs: 124 success, 13 precise overflow, 7 precise fuel exhaustion
- 288 ordinary standalone LLVM artifacts with profile-identical hashes
- 120 seeded stateful/nested-loop sources plus fixed reset/type/call/lazy/return
  and unused-cycle cases
- Exact default 1,000,000-operation success and 1,000,001-operation failure
- Twelve negative source cases checked through check/run/compile before tools
- Seven guarded native resource cases and eight additional artifacts: inclusive/+1
  4,096 blocks, 8,192 slots, 32 frames; guarded 32-frame/64-user-argument execution
  under a 1 MiB process stack
- 1,268 compiler invocations in the complete gate
- Model witnesses: 9,803,334 charged operations, 1,310,603 loop iterations,
  682,196 stores; these are expected model counts, not exported runtime tracing

Reference JSON/text, exact byte ranges and human error origins, exits, source-absent
clean-PATH execution, ELF/libc checks, `/dev/full` and broken-pipe status 74 agree.
All seven resource cases passed, including real artifacts at admitted boundaries
and E0700 before tools/output at the adjacent rejected bounds. The new while
oracle plus cyclic raw LLVM suite executed 412 native artifacts (296+116),
excluding 40 predecessor raw artifacts.

Corpus SHA256:
`a35dde5a5fd3baf6bf5dbb4eec4f24b19ee20ad39e4aaa4276a7be494e9c3125`

Artifact-manifest SHA256:
`d606434c57b2318a9c995bfc8098289a2e8bd8ca6bfc9d35a807c3393687b37e`

Model-trace SHA256:
`19eab5995d27a441daa5e31edd27e3fea64c48bc64d4acfc394de46d0ee8e535`

## Compiler and regression gates

Fresh formatting and strict Clippy passed. Both full all-target/all-feature
profiles passed 387 ordinary tests (263 unit +124 integration), zero failures and
three explicit LLVM ignores separately executed as above. Both compiler builds
passed. Reference arithmetic/literal and metadata gates passed. The repository audit passed 121 sources and 67 runnable programs, and metadata
validates 23 entries with seven unit tests. All complete predecessor native gates
passed with compiler hashes unchanged before/after the chain:

- Mutable locals: 163 sources, 326 ordinary+10 resource artifacts, 49 negatives,
  thirteen native resource cases and seven reference-only cases
- Boolean logic: 525 sources, 1,050 ordinary+14 resource artifacts, 149 negatives,
  eighteen resource cases and seven reference-only cases
- Scalar comparisons: 1,610 sources, 3,220 artifacts, 198 negatives, fourteen resource
  cases and 14,124 invocations
- Native arithmetic: 1,051 sources, 2,102 artifacts, 605 success and 446 overflow
- Original scalar gate: 57 real artifacts, 32-frame/64-argument 1 MiB-stack stress,
  ELF/PIE/libc, output failures, reproducibility and injected tool/adapter failures
- Reference arithmetic: 4,100 invocations over 1,025 sources; literals: 1,670 invocations

The mutable source corpus intentionally replaces its former unsupported-while
negative with still-unsupported match. Its new corpus hash is
`a403f3076476677ea062e41f6eda7ca3f94d68192e25abd0ecd9659c088c9528`;
its artifact/trace hashes and all counts remain unchanged. No predecessor positive
coverage was removed.

A separate independent review reran all 387 ordinary tests/profile and all three
explicit raw LLVM tests/profile. Its own 66-source/132-artifact probe covers 64
seeded nested state/call/snapshot cases, an independently hand-counted empty-loop
fuel origin and unused cyclic work with loop-free main. It passed with the same
compiler hashes and reported no material finding. Exact publication identity and
hosted CI are separate final gates; no remote result for this candidate is claimed.

Debug compiler SHA256:
`2ffa3063770519fd9aa3d1e06316f573f4f24b87b9ef791ab0c85250144e16cf`

Release compiler SHA256:
`60ef09f9b3a74e73f7337634e918c841d3ce1a50ceaed5b73b73f3573751d6aa`

Required commands (all from repository root with the pinned LLVM environment):

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
cargo test --locked --bin oxid frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm -- --ignored --exact
cargo test --release --locked --bin oxid frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm -- --ignored --exact
python3 scripts/verify_while_loops.py target/debug/oxid target/release/oxid
python3 scripts/verify_mutable_locals.py target/debug/oxid target/release/oxid
python3 scripts/verify_boolean_logic.py target/debug/oxid target/release/oxid
python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/debug/oxid target/release/oxid
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/verify_feature_status.py
python3 scripts/verify_repo.py target/release/oxid
```

The native CI job explicitly invokes all three raw LLVM tests in both profiles
and the new while oracle in addition to all predecessor oracles. Qualification
remains provisional Linux x86_64 LLVM 19.1.7 O0; no native recursion, escaping
references, ownership, stable edition, performance parity or final release claim.
