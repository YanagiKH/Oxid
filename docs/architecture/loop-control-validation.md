# Unlabeled loop-control validation

This report qualifies the experimental break/continue slice described by
[RFC 0013](../../rfcs/0013-loop-control.md). Base: PR16 merge
`d95ed4fc0ad2590d64d251f85e4619c7049041e6`, tree
`a9deca7e25fc801dc61dac9d7a11c50e924467a2`. The new candidate's exact tree and full
payload are recorded in the publication/review evidence, not self-referentially
embedded in this file. No milestone, universal safety or other-target native
qualification follows from these tests.

## Host and tools

Local Linux x86_64, Rust/Cargo 1.98.1, Debian LLVM/Clang/LLD 19.1.7
(`1:19.1.7-3+b1`). Both Rust compiler profiles emit native O0; this does not test
LLVM O2 or call it release-native optimization. Resulting ELF PIE programs run
without source files, Oxid, Rust, Python or LLVM tools; libc/loader remain their
declared runtime dependencies.

Stable tested compiler SHA256 values:

- debug: `e12436e17a643ec89fd8ce9fe79ca34eef7395b89aa133c64c35f1853f60f890`
- release: `cbaa96e7721ebc352e61ffa67372d93a5ad4eba71b51ecb8bdda294e5978a768`

Hosted CI for this new candidate is not claimed by this local report. The merged
predecessor's CI is separate evidence and cannot substitute for candidate CI.

## Core and public evidence

The clean predecessor passed 387 ordinary Rust tests. A valid break source then
failed with its predecessor E0101; the same new public group passes after the
implementation. Supplemental tests were added after that initial red/green;
not every final assertion was independently observed red.

Fresh formatting, strict Clippy and full Rust suites pass in debug and release:
404 ordinary tests per profile (275 unit + 129 integration), plus four explicitly
ignored Linux LLVM tests separately executed in both profiles. Integration counts
by binary are 12,1,8,8,6,10,8,19,8,9,5,11,18,6. No predecessor tests were removed;
two obsolete unsupported-break/continue negatives become invalid value-bearing
transfer negatives with E0100.

New focused coverage includes:

- Four-outcome flow composition exhaustively compared for 16×16 outcome sets
- Sequential terminal-outcome preservation, mixed return/break/continue arms,
  missing else, conservative while false edges, return-only message preservation
- Lexical target restoration across nested/sibling loops and function boundaries;
  invalid nearest-loop producer identities rejected by release assertions
- Exact OIR preflight/emission agreement for 20 arm-shape combinations; no fake
  joins or extra closing-brace edges after transfers
- Continue targets the original condition header before calls/lazy joins
- Full dead-body checking, forbidden value/label/expression forms, mandatory
  semicolons, active-name scopes, reused locals and inclusive 63/64 nesting cuts
- Full statement diagnostic spans including Unicode/comments/CRLF, static errors
  before tools/output, and fuel-bounded infinite continue

## Independent exact-fuel and actual LLVM evidence

Three hand-written source-operation schedules cover break completion at 9 fuel,
continue-counter completion at 37, and checked overflow reached at 12. Every lower
budget has its next full source origin predicted independently of compiler OIR.
The overflow witness checks fuel failure immediately before the arithmetic
charge and E0604 only after that charge succeeds. Internal transfer trivia is
included in the span. An unused cyclic helper forces guards even for a break-only
main, so exact native cutoff testing cannot silently use unguarded admission.

The mandatory native test creates and executes 77 artifacts per compiler profile:
61 cutoff/failure artifacts plus eight semantic source shapes in original and
reversed/nonzero-entry block order. Expected scalar results or exact E0601/E0604
human diagnostics are asserted. The path includes newline/tab/Unicode bytes,
checking escaped embedded diagnostics. Missing LLVM fails this explicit test;
ordinary ignored-test totals are not counted as its execution evidence.

Both profiles passed: 154 new artifacts. Existing bool-merge, mutable-place and
raw-cyclic LLVM tests also ran explicitly in both profiles, adding 156 predecessor
artifacts and 12 intentionally invalid phi rejections. Thus all four raw/test
families execute 310 artifacts across both profiles. These are separate from the
source-oracle artifacts below.

Native classification tests confirm that actual CFG cycles determine guards:
break-only and return/break bodies may be acyclic; continue or a falling body
retain backedges; an unused cyclic helper guards the whole module. The native
emitter, runtime executor, verifier algorithm, adapter/tool trust and no-clobber
implementation are unchanged by this slice.

## Independent source oracle

`verify_loop_control.py` reuses only earlier independent scalar expression/
rendering primitives and supplies its own static outcome checker and dynamic
transfer machine. It does not read compiler IR, derive expected values from the
compiler, or use legacy dynamic semantics. A transfer exception is caught only by
the nearest model while; explicit transfers are charged and skip arm/body closing
edges. Model scope and initialization reset separately on each body execution.

The corpus contains 186 modeled sources: 144 seeded nested-state cases, all 18
selected pairs of return/break/continue arm outcomes, scalar/unused-cycle cases,
condition calls and lazy errors, skipped/executed overflow, infinite continue,
shared call budgets, and exact default 1m/one-over budget sources. Generated cases
return a checksum exposing both arithmetic and accumulated inner-loop/snapshot
state. Model traces are expected witnesses, not claims that CLI execution emits a
trace. Observable results, failure code/full byte span/human text and artifact
parity are checked against reference/native execution.

There are 35 invalid sources tested through check/run/compile, including static
errors before deliberately missing tools, and seven native resource cases for
4096/4097 blocks, 8192/8193 slots, 32/33 frames and guarded 64-user-argument calls at a
1 MiB stack. Successful resource cases add eight native artifacts. The complete
source gate therefore runs 372 ordinary +8 resource artifacts = 380, with 1742
compiler invocations. Both profiles must emit byte-identical artifacts. Source
files are absent during native execution; clean PATH, ELF libc dependency,
/dev/full and broken-pipe exit 74 are checked for i32/bool/unit/E0601/E0604.

The final refined oracle passed all 186 cases: 172 success, 10 overflow and four
fuel exhaustion. It modeled 5,064,939 charged units, 900,125 loop iterations,
234,777 stores, 391 breaks and 899,288 continues. A prior oracle iteration returned
only arithmetic state; the final checksum refinement exposes accumulated state
as well and is the version qualified here.

- Corpus SHA256: `3795a3da71d56708eb08df86c4998fd773b238d55d76f62b5865f0cb5a318fc3`
- Artifact manifest SHA256: `8827b2b90d5d1fd7a7e7ece614763b48887013617bec08bb83201bdab80120d9`
- Model transfer/store trace SHA256: `ec32ea5c1bdbf22ea1e0475399419e758492c6758ea000185b89dc24f8f2df0b`

The new slice totals 380 source/resource + 154 cutoff/permuted artifacts = 534
actual native executables across both profiles. This excludes predecessor
artifact counts.

## Full regression qualification

All six predecessor native gates passed against the stable binaries:

- Native scalar/tool-boundary suite: 57 real compiled cases, including narrow
  stack, libc/I/O faults, reproducibility and no-clobber behavior
- Checked arithmetic: 1,051 sources / 2,102 native artifacts
- Comparisons: 1,610 sources / 3,220 native artifacts, 198 negatives,
  14 resource cases and 14,124 compiler invocations
- Boolean logic: 525 sources / 1,050 ordinary + 14 resource artifacts,
  149 negatives, 18 resource cases, seven reference-only cases and 5,236 invocations
- Mutable locals: 163 sources / 326 ordinary + 10 resource artifacts,
  49 negatives, 13 resource cases, seven reference-only cases and 1,718 invocations
- Ordinary while: 144 sources / 288 ordinary + eight resource artifacts,
  12 negatives, seven resource cases and 1,268 invocations

The while corpus hash changes to
`cbd9d8a31f9987945354e45db3bd83adb15cdc1bf758a59f97fcb08cfcb9f3ab`
solely because its two obsolete unsupported-transfer negatives become invalid
value-bearing transfers. Its artifact manifest remains
`d606434c57b2318a9c995bfc8098289a2e8bd8ca6bfc9d35a807c3393687b37e`
and model trace remains
`19eab5995d27a441daa5e31edd27e3fea64c48bc64d4acfc394de46d0ee8e535`.
Boolean/mutable corpus, artifact and model hashes remain unchanged. Existing
positive source behavior and native emission are preserved by these comparisons.

Reference arithmetic (4,100 invocations), literals (1,670), all seven metadata
unit tests, the 24-entry feature inventory and 121-source/67-runnable repository
verification passed. Workflow YAML parses and all eight explicit LLVM profile
commands resolve to real registered tests. All core/source/test/oracle hashes
and both compiler hashes were rechecked after the entire chain; no stale-binary
substitution is part of this qualification.

## Independent review evidence

A separate reviewer read the production parser/resolver, outcome composition and
scoped lowering diff, then ran a distinct 67-source probe: 64 seeded nested
transfer/state programs, two mixed-return helper cases and one independently
hand-counted infinite-continue fuel origin. Its 134 real native artifacts matched
reference/model results, compiler-profile hashes and source-absent execution.
The reviewer also repeated all 404 ordinary tests and all four explicit LLVM
tests in both profiles against the stable compiler hashes above. No material
finding was reported in that review; final exact-payload review remains a
separate publication gate.

## Reproduction

With pinned tools in `OXID_LLVM_BIN` and the Rust toolchain on PATH:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
cargo build --locked
cargo build --release --locked
for test in \
  frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm \
  frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm \
  frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm \
  frontend::oir::loop_control_tests::loop_control_uses_real_llvm
do
  cargo test --locked --bin oxid "$test" -- --ignored --exact
  cargo test --release --locked --bin oxid "$test" -- --ignored --exact
done
python3 scripts/verify_loop_control.py target/debug/oxid target/release/oxid
python3 scripts/verify_while_loops.py target/debug/oxid target/release/oxid
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

These bounded tests do not prove termination, ownership/alias safety, all source
mappings or LLVM correctness. Labels, value-producing loop transfers, stored
references, non-Copy types, destructor/error cleanup and native recursion remain
unavailable. No M2 proposal is implemented or approved by this work.
