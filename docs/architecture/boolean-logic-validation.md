# Boolean logic and short-circuit joins: local validation

Scope: the accepted bool-only `!`, `&&`, `||` contract in
[RFC 0010](../../rfcs/0010-boolean-logical-operators.md), based on merged main
`8dd37229bcc09351833045a021ae6d333530f8d2` (tree
`eeb3695096e9af0550fb4ec7328af534095383ad`). This report distinguishes executed
local checks from review/hosted-CI gates. It is not a stable-edition, ownership,
production-safety or completed-roadmap claim.

## Environment and implementation boundary

2026-10-01, Linux x86_64, Debian 13/glibc 2.41, Rust/Cargo 1.98.1, pinned
LLVM/Clang/LLD 19.1.7. Debug/release refers to the Rust compiler build; both emit
native code at O0. No O2/LTO or non-Linux native result is claimed.

Source grammar/types, root-directed OIR lowering, explicit bool merges, independent
edge verification, reference predecessor selection and LLVM xor/branch/phi lowering
change. The private scalar/error ABI, C adapter, tool trust/invocation boundary,
no-clobber output publication, all resource ceilings, legacy execution and OXBC
remain unchanged. The earlier arithmetic/comparison unsupported-token controls
are updated for newly accepted logical tokens while retaining invalid bitwise,
textual logical and separated-comparison controls. Recognized `!` in an invalid
position now correctly reports E0100 rather than its former unsupported E0101.

## Recorded red/green boundaries

The focused public CLI test first failed against the frozen comparison binary
at missing `!` syntax, before implementation. After expansion, the complete
nine-test suite was also independently compiled against that unchanged baseline:
all nine failed with exit 101 at missing logical syntax. The tagged lazy oracle
separately failed at `!true`/E0101. These are recorded baseline checks; not every
later adversarial case is claimed as an individual tests-first cycle.

After frontend/root-directed lowering and runtime support, the merge-positive
source test failed E0500/Uninitialized at verification. Adding a canonical merge
definition and edge-read verification made the same test pass. Native tests then
failed E0700 at the still-closed native operation allowlist. Explicit xor/phi
lowering/admission made them pass and two initial actual LLVM smoke programs
returned the expected false/true values, including joins after checked arithmetic.
Additional adversarial raw and integration cases were added against those working
interfaces; they are test coverage, not separate claimed baseline-red cycles.

## Independent verification and exact budgets

Nine raw-OIR test groups cover exact predecessor-edge membership, forbidden entry
merges, missing/extra/duplicate edges, wrong IDs/types/spans, global duplicate
definitions, self/other-arm uninitialized inputs, same-block use after entry merge,
normal call-result edge availability, own-call-argument rejection and source
operator origins through Unicode/CRLF. They include a direct call return into a
merge with both runtime branch choices and emitted phi inputs. A separate exhaustive
small-DAG/path-removal oracle checks incoming assignment/call-edge availability
under original and reversed block IDs, without using the production dominators.

Assignment budget checks include 100,000 ordinary assignments plus a merge
(rejected before definition traversal), and 99,999 plus a merge (admitted by
budget preflight, then rejected for deliberately duplicated raw definitions).
Existing long/wide CFG, source/token/depth and dominance regressions remain
enabled. The independent 100,000-token boundary is exercised with 33,329 logical
statements; this public grammar reaches that cap before independently isolating
the unchanged 100,000-syntax-node ceiling. Raw OIR assignment bounds are tested
separately as above.

Three reference tests hand-count 6 fuel for unary negation, 8 for a direct
short-circuited logical expression, and 10 for its evaluated-RHS path; one less
fails at Return and two less at merge entry. Grouping and full-function local
allocation retain their old costs. Exact event traces cover all eight logical
truth-table paths and skipped/executed recursion. Two native unit groups verify
conservative cost, whole-call-graph rejection and phi predecessor labels after
multiple checked-success blocks. Two CLI tests verify pre-tool admission and full
checking/native recursion rejection for skipped RHSs.

A Linux raw-IR integration test is explicitly ignored in ordinary toolchain-free
host suites and mandatory in the pinned native CI job. Explicit runs fail if the
pinned tools are absent. It passed in both compiler profiles and produced 16 real
standalone executables: direct call-result incoming edges, both branch values,
checked arithmetic before the call and reversed/nonzero-entry block IDs. Four
mutated modules with wrong post-overflow phi predecessor labels were rejected by
the real LLVM verifier before artifact publication.

## Independent lazy source model

The Python evaluator uses explicit scalar tags, arbitrary-precision per-node i32
checks and branch selection before RHS evaluation. Its self-checks cover call
truth tables, skipped poisoned arguments and left-error suppression. The corpus
includes 320 seeded lazy trees and dedicated call/checked/nested predecessor
cases, plus mixed chains, discarded values, branch consumers, static negatives,
Unicode/CRLF origins and precise source/token/height/resource boundaries.

Source-model call traces are independent expected witnesses. Public CLI checks
observe their consequences through values and unique first-overflow origins;
there is no exported runtime tracing API. Separate private reference tests
observe actual function/branch events. Native output, exact human E0604 errors,
artifact hashes and source-absent execution are compared in both compiler profiles.

## Local code gates

- Strict Clippy and both compiler builds passed
- Full all-target/all-feature Rust tests passed 334 in each profile:
  229 unit tests + 105 integration tests, zero failures. One LLVM-required unit
  test is ignored in each ordinary Linux suite and separately passed explicitly
  in each profile, as described above
- Public logical CLI tests passed all nine against reference execution
- Complete lazy tagged Python/reference/native gate: 525 sources (380 values and
  145 exact overflow results), 1,050 native corpus artifacts plus 14 genuinely
  compiled/executed resource-boundary artifacts, 149 negative sources, 18 native
  admission cases and seven reference-only cases passed; 5,236 compiler invocations
- Both compiler profiles produced identical artifact hashes; all standalone
  source-absent executions, ELF checks and output-failure-74 cases passed. Combined
  with the 16 raw-IR executables above, this slice exercised 1,080 actual native
  artifacts. The source corpus SHA256 is
  `787a23e39c1685cf53df1a7374ccd1362bc5a753b26b6b7ae05e4418748d49f3`
- Metadata unit tests: seven passed; feature inventory: 21 entries valid
- Reference arithmetic predecessor: 4,100 invocations / 1,025 sources passed
- Exact i32 literal predecessor: 1,670 invocations passed
- Repository verifier: all 121 sources / 67 runnable programs passed
- Native comparison predecessor: 1,610 sources / 3,220 artifacts, 198 negatives
  and 14 resource cases passed, preserving exact profile hashes and diagnostics
- Native arithmetic predecessor: 1,051 sources / 2,102 artifacts passed
- Scalar native predecessor: 57 compiled cases plus fault injection, output
  failures and 32-frame/64-argument execution at a 1 MiB stack passed
- Formatting, final strict Clippy, metadata/path checks and workflow YAML parsing
  passed after integration; both compiler binary hashes stayed unchanged through
  the complete differential runs

## Reproduction

With the qualified LLVM environment available:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
python3 -m unittest discover -s scripts -p 'test_*.py' -v
cargo build --locked
cargo build --release --locked
cargo test --locked --bin oxid frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm -- --ignored --exact
cargo test --release --locked --bin oxid frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm -- --ignored --exact
python3 scripts/verify_boolean_logic.py target/debug/oxid target/release/oxid
python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 scripts/verify_repo.py target/release/oxid
```

The new CI native step retains every predecessor gate. CI configuration alone is
not evidence of a hosted run for this logical increment. Independent review,
publication and hosted CI are separate; no such result is claimed here.
