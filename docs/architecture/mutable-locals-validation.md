# Mutable scalar locals: local validation

Scope: the accepted initialized, fixed-type scalar mutation contract in
[RFC 0011](../../rfcs/0011-mutable-scalar-locals.md), based on merged main
`612155e1da4789dceb76cedae98705408c18a0c4` (tree
`55d2096085ac7155bc70736a9dc385ef99638203`). The implementation began in an
isolated worktree on the byte-identical published PR14 tree, then reconciled to
that merge without changing source bytes. This is local evidence, not a stable
edition, ownership certification, completed milestone or hosted-CI result for
this candidate.

## Environment and scope

2026-10-01, Linux x86_64, Debian 13/glibc 2.41, Rust/Cargo 1.98.1 and pinned
LLVM/Clang/LLD 19.1.7. Debug/release describes the Rust compiler build; both emit
native O0. No O2/LTO or non-Linux native validation is claimed.

Changed boundaries: mutability-aware source/HIR checking, separate typed mutable
places, explicit initialization/store/load, independent initialization dominance,
reference frame storage and private native scalar allocas. The existing adapter,
ABI/error output, tool validation, no-clobber publication, libc dependency,
resource ceilings, legacy execution and OXBC format remain unchanged. Existing
raw SSA fixtures now explicitly select Assign from the statement enum, preserving
their original assertions. Former unsupported-mutability controls are replaced
by const/bitwise controls; new public tests cover accepted mutation and rejected
immutable targets. Recognized `=` in invalid expression positions now produces
E0100, and predecessor comparison tests/oracle expectations record that change.

## Focused and adversarial evidence

A clean baseline passed 334 ordinary Rust tests before implementation. One
focused public test failed against the frozen predecessor with E0101 on `mut`;
the complete eleven-test new suite also failed against that binary. The
independent oracle likewise failed on the missing syntax. The native admission
slice recorded E0700 before storage lowering, then reached the expected missing-
tool E0701 boundary after implementation. Later raw adversarial cases were added
after the vertical slice; not every later check had a separately demonstrated
predecessor failure.

Eleven public test groups cover all scalar types, immutable snapshots, repeated
stores, both/missing/returning arms, closed-scope name reuse, no-active-shadowing,
mutable conditions, short-circuit RHSs, copied arguments and independent recursive
activations, checked-overflow first origins, fixed-type/dead-code errors,
immutable targets with declaration labels, malformed assignment forms, 4,000
ordered stores and expression-height boundaries.

Eight raw OIR groups independently cover separate place/value namespaces,
canonical missing/duplicate initialization, unchanged SSA uniqueness, pre-init
and forward-RHS reads, call-result availability, invalid IDs/types/UTF-8 spans,
exact Unicode/CRLF statement/target/operator origins, and inclusive slot and
instruction caps. The caps count places, initialization and stores, including
100,000 combined declarations and 100,000 instructions. A graph oracle compares
170,490 initialization-availability decisions for store-only, load-only and combined access to independent path removal over
all reachable small forward DAG fixtures of at most six vertices and at most two
outgoing edges, including reversed IDs/nonzero entry. This is exhaustive over
that stated fixture family, not arbitrary CFGs.

Three private reference groups independently hand-count place allocation,
init/store/load fuel and call-frame/live-slot totals, observe exact state/call/
branch events, and verify no Store event follows RHS overflow or exhausted store
fuel. Direct storage-seam tests preserve the previous scalar on invalid type or
duplicate initialization. Two native unit groups verify combined slot/cost
accounting and typed private alloca/load/store output without changing immutable
SSA snapshots. A native CLI group proves admission, full unchosen-branch checking
and recursive-call rejection precede tool invocation or output creation.

A new Linux-only ignored raw test is mandatory in the pinned native CI job.
Explicit invocation fails if LLVM is missing. It passed in both profiles,
producing 24 standalone executables across bool/i32/unit, both branch choices,
checked arithmetic/calls/logical joins and ordinary/reversed OIR block order.
This specifically checks private allocation dominates nonzero-entry CFGs. The
predecessor raw bool-merge test also passed explicitly in both profiles: sixteen
executables and four deliberately wrong-phi modules rejected by real LLVM.
Ordinary toolchain-free suites do not silently count these ignored tests as run.

## Independent state-model oracle

The source oracle uses explicitly tagged Python scalars, lexical environment
frames, mutable cells, immutable snapshots and per-call isolated state. It
chooses if/logical paths before evaluating skipped expressions, and checks
arbitrary-precision i32 results at each executed arithmetic node. Operator
origins are calculated while rendering independent model trees. Self-checks
establish its own state/call/branch behavior before comparing compiler outputs.

The final complete gate passed for both frozen compiler binaries:

- 163 modeled source programs: 148 successes and 15 precise overflows
- 326 ordinary native artifacts plus ten real resource-boundary artifacts
- 49 negative source cases across check/run/compile
- Thirteen native resource cases, including inclusive/+1 per-function and
  aggregate slots, functions, call depth and exactly 100,000/+1 fuel
- The exact-fuel base explicitly includes initialization, Store and Load
- Seven reference-only cases, including 4,000 stores, the 100,000-token boundary
  and recursive activation/resource behavior
- 1,718 compiler CLI invocations; model witnesses include 961 stores,
  407 calls and 252 branches

All reference text/JSON results, exact E0604 origins and human stderr, exits,
profile artifact hashes, source-absent clean-PATH execution, ELF/libc checks,
`/dev/full` and broken-pipe exit 74 checks passed. The model trace totals are
independent expected witnesses, not an exported trace from the CLI; actual
reference state events are separately tested by the unit-only observation seam.

The mutable gate plus the new raw LLVM test executed 360 native artifacts.
The separate sixteen predecessor merge artifacts are not included in that total.
The public grammar hits its token bound before independently isolating the
unchanged syntax-node ceiling; the raw OIR instruction cap is tested separately.

Corpus SHA256:
`0b0645f87350f8635aad8365c583b9c7495f998fcc3d487a3bce2eb5a11fd99b`

Artifact-manifest SHA256:
`64e671d4e6523ce23350bb68f85cea0f4be3eba1eab7e588747ee491361e4222`

Source-model trace SHA256:
`37caf97ae9362ba59ffdd6873f73e233819dcfd467f4021a823e08be9a6e18eb`

## Compiler gates

Formatting and strict Clippy passed. Full all-target/all-feature Rust suites
passed 359 ordinary tests per profile: 242 unit +117 integration, zero failures,
with two Linux LLVM-dependent ignores separately executed as described above.
Both compiler builds passed. New public tests also passed independently against
each frozen compiler binary. Reference arithmetic passed 4,100 invocations over
1,025 sources; exact-literal checks passed 1,670 invocations.

Debug compiler SHA256:
`22a7706491f7323b95d8facdb9c3fa744bb220ada1dac37a4333e6961234e8fe`

Release compiler SHA256:
`d5ed4d47ea236e141a874f90ee497f19f50ff8f2f9db5ab782b8cbdc15084623`

All complete predecessor native gates passed with unchanged compiler hashes:

- Boolean logic: 525 sources, 1,050 ordinary +14 resource artifacts, 149 negatives,
  eighteen native resource cases and seven reference-only cases
- Scalar comparisons: 1,610 sources, 3,220 artifacts, 198 negatives, fourteen
  native resource cases and 14,124 compiler invocations
- Native arithmetic: 1,051 sources, 2,102 artifacts, 605 successes and 446 exact
  overflows, including adapter fault-injection and output-failure checks
- Original scalar native gate: 57 real compiled cases, reference/Python parity,
  32-frame/64-argument stress with a 1 MiB stack, ELF/PIE/libc, I/O and reproducibility

The repository audit passed 121 sources and 67 runnable programs. Seven metadata
unit tests passed and the inventory validates 22 entries. YAML parsing and the
mandatory native test/oracle commands were checked. All 26 stable code/test/
oracle/CI file hashes and both compiler hashes were rechecked after the gates.

A separate read-only review exercised 96 independently seeded stateful sources
(seed 878312), 192 real native artifacts and six invalid-source cases. It covered
snapshots, helper copy-by-value, branch-local state and lazy bool updates, with
reference/model/native and compiler-profile artifact parity. Fresh full Rust
suites and both mandatory raw LLVM tests passed in both profiles during review.
No material source/probe finding was reported against the stable implementation.
Final serialized-payload identity review and hosted CI remain publication gates.

## Reproduction commands

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
cargo build --locked
cargo build --release --locked
# OXID_LLVM_BIN points to the pinned LLVM 19.1.7 bin directory.
for profile in '' '--release'; do
  cargo test $profile --locked --bin oxid frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm -- --ignored --exact
  cargo test $profile --locked --bin oxid frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm -- --ignored --exact
done
python3 scripts/verify_mutable_locals.py target/debug/oxid target/release/oxid
python3 scripts/verify_boolean_logic.py target/debug/oxid target/release/oxid
python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/verify_feature_status.py
python3 scripts/verify_repo.py target/release/oxid
```

Independent review and hosted CI remain separate publication gates. The feature
inventory keeps this extension experimental, with no automatic target expansion,
release promotion, loop support or ownership claim.
