# Owned source integration: qualification ledger

Status: **experimental; specified local qualification gates pass**. The production
source route implements [RFC 0014](../../rfcs/0014-owned-structs-call-borrows.md).
Both final public CLIs pass the complete source/ELF corpus; the separate portable
native pipeline passes its bounded fuel, semantic-store, physical-guard and
artifact checks. Final ordinary and explicit native/resource suites, privacy,
Python, scalar regression, repository, formatting and strict Clippy gates pass.
Independent activation, native matcher and portable pipeline reviews pass within
their stated scopes. Hosted CI has not run for a publication head. These are
bounded local results, not unrestricted language or memory-safety certification.

The implementation baseline is main
`cb370e5ec1261d99c80fae88979aa9bf4e45d8f0`, tree
`c77beb59b5d66dba974ca38743a913c8a9dcbee1`. This is the pre-source-integration
baseline, not the final activated source identity. The historical
[raw-consumer validation](owned-consumers-validation.md) remains evidence for its
own frozen raw fixtures. Its 522 ordinary tests/profile, 196 LLVM/ELF artifacts
and raw Batch's 1,086 fuel are not source-integration counts or results.

## Scope and evidence identities

The source subset is nominal move-only scalar-field/empty structs, complete
moves/replacement, scalar field access, owned helper returns and explicit
exact-mode call-only borrowing/reborrowing. All declarations and skipped paths
are checked. Any owned syntax selects one owned route for the complete module;
scalar-only modules retain their existing route. Neither an error nor native
admission rejection can fall back to scalar, legacy or reference execution.

Stored/returned references, nested owned fields, partial moves, field-disjoint
loans, standalone blocks, general dereference, scalar/field/temporary borrows,
reference coercions, mutable parameter bindings, heap/destructors and unsafe/FFI
contracts are excluded. Check may accept an owned-returning main; run/compile
require a zero-argument scalar main. Native scope remains Linux x86_64,
`x86_64-unknown-linux-gnu`, LLVM/Clang/LLD 19.1.7, O0. No other native target,
optimized-code qualification, stable ABI, memory-safety certification, M2 or
v1.0 completion is implied.

| Identity | Recorded value |
| --- | --- |
| Activated source inputs | [240-file source/test/script/workflow manifest](owned-source-inputs.json), SHA-256 `f00773e6cbb19eba6d08a68f9189a1ec7ece1a61d3d10826d43780fe20492f11`; baseline identified above |
| Production parser/driver/source dispatcher | Activated implementations and their hashes are bound by the input manifest; independent activation review `980dc4d38d2f09e82e63330ba7ca8c04588f122530612927e7d6dc5a85ec3ec6` passes the scope described below |
| Host and compiler toolchain | Linux 6.18.44 x86_64, glibc 2.41; Rust 1.98.1 (`48a229cea`, 2026-09-01), Cargo 1.98.1 (`797e8a9bc`, 2026-08-05) |
| Native tools | LLVM/Clang/opt/LLD/llvm-as 19.1.7, Debian packages `1:19.1.7-3+b1`, O0; tool identity report SHA-256 `5ab519e296a27c5b4a7d67e292c07692a6e2143c0f4496737f70a793f0420afa` |
| Debug CLI SHA-256 | `e3244358947bc9389a0a1d55bd18a3a8a96c223dd629b79fda0a440f07951ded` |
| Release CLI SHA-256 | `7a7c32e78aac371e49f5b728514b79ea6e71bf35e89b3bc7810073e89536878d` |
| Debug native collector test executable SHA-256 | `8ee391ddc67c95d0afb238f9fcc7911e3e653c153cbf6e59dd428f2c9d55bf6c` |
| Release native collector test executable SHA-256 | `92eeb6a13dc5cfe623b8f50607caa46d9d10dad0f57a5ac9d89bfca869deea56` |
| Source model / CLI harness SHA-256 | `5c2a74e41aaadaf5706fb0ea35931328317e906a05b9d26776837aedddc8dfc7` / `cda43ff8a99b4b9f713a9228629b1e96c1e5dd7e58157becf133c22aac8aa266` |
| 431-source frozen expectation manifest SHA-256 | `297609147384f087a6252645f8f496f01f56132703fdcd45553b1d27b39a3ccc`, unchanged from the reviewed amendment |
| Batch source SHA-256 | `dbbadef9a035e3af62aab6ae2ca0ac19531684e4ad8ef25bc4d81a4fa7c07db2`, exact bytes at [`fixtures/owned_source/batch.ox`](../../fixtures/owned_source/batch.ox) |
| Matcher and review SHA-256 | Matcher `e4a5e3d71bd04f604487b385412e23e96bcb07d1142c473429140550dd9155e2`; bounded matcher review `450d0e6e8cc336d29e7bbfdc050ed48bd67aad576bd9a5475d54b776451e0a58` |
| Publication-head hosted CI | Not yet run; local evidence and configured workflow coverage do not establish hosted success |

The retained toolchain report binds versions and executable hashes from the same
qualification environment. The corresponding reproduction commands, with
`OXID_LLVM_BIN` pointing to the trusted pinned tool directory, are:

```sh
uname -a
rustc --version --verbose
cargo --version
"$OXID_LLVM_BIN/clang" --version
"$OXID_LLVM_BIN/opt" --version
"$OXID_LLVM_BIN/ld.lld" --version
git rev-parse HEAD HEAD^{tree}
git diff --stat
sha256sum target/debug/oxid target/release/oxid
sha256sum scripts/owned_source_model.py scripts/verify_owned_source.py \
  scripts/test_owned_source.py fixtures/owned_source/batch.ox
```

The public input manifest binds the activated unpublished source rather than
relying on the baseline HEAD alone. Preactivation receipts, final CLI results and
portable native observations have separate identities. Earlier-binary results
remain scoped to their recorded snapshot.

### Publication provenance normalization

One metadata-only fixture change followed the native run: host-specific collector
paths in `tests/fixtures/owned_source/producer-translations.json` became portable
provenance labels, with original collector-manifest hashes retained. The nine
source cases, expected values, exact receipt JSON strings and receipt hashes are
unchanged. Its fixture SHA-256 changed from
`d578390f6925256150935fbe3cfcde52aa47648cbbda0c74305a094371dbb653` to
`efaa8b857ab0d9fb1ab1ca65555e1e170f01e1e6314172e44ca462d55f181ed2`.
The original collector manifests remain identified by
`6346e86fc9ef75452149ee9bfaf46acc665806f059127b9311b5e16e4d387fc5`
and `c120c0f1cf0dcfa596f7c0e4f796ac454f8b3f4a50e82ad1f02b09603d813806`.
All 18 affected receipt tests pass after normalization.

Exactly this one entry changed in the public 240-file input manifest; the other
239 hashes, compiler/model/harness/matcher/RFC bytes and native execution inputs
are unchanged. The actual native result remains bound to its original snapshot,
while the public input manifest binds the normalized publication metadata. The
independent packaging supplement passes: exactly 32 path-label replacements plus
the original-manifest provenance object; all nine cases, receipt strings/hashes
and 18 archived receipts are unchanged, both archived manifest hashes verify,
and all 18 affected receipt tests pass independently. Review-manifest SHA-256 is
`8619bd8c0451adec2d4e33c2ad4d9868f365bea1d2d8abae7575dd4eaeac6319`.
This is a reviewed packaging delta, not a native rerun or a semantic change.

## Retained preactivation evidence

These results were obtained before public parser/driver activation. They establish
bounded implementation and test evidence for the recorded snapshots, not final
production CLI, final runner or exact-head hosted CI results.

| Historical scope | Retained result and identity |
| --- | --- |
| Dormant compiler/resource-complete snapshot | 696 ordinary tests passed and 21 ignored across 16 suites in each profile; formatting and strict all-target/all-feature Clippy passed. Source/context manifest SHA-256 `75076f45f80fedb0d220ba49d330135f69167e848223dedb34377880c4ed5a24`; release test executable `2148fa7f5869138f8d608c963c88067233e6298c8edad67de3236422edc07ef5` |
| Independent source model | 431 unique sources: 133 check-accepts and 298 rejects. Amended frozen manifest `297609147384f087a6252645f8f496f01f56132703fdcd45553b1d27b39a3ccc`; review `6d17a1ea5e83cefed0a828105962f5d7c022f2e3c55416b4ab36a6ce080dcfd7` |
| Source-facade comparisons | Debug, release and repeat-release each compared all 431 receipts, including 4,525 ordered charges and 216 alias-family selections; complete receipt bytes agreed. The final reviewed adapter performed 24,811 comparison checks per profile with no mismatches |
| Corrected receipt adapter | 129 integrated and 19 independent methods; 308 object-shape, 295 array-type and 8 strict-JSON controls. Reviewed manifest `1ac5fc2b66720fdca96a51be731f703dcb6787e6f8e8b73b0123a532a550ba72` |
| Actual source translation controls | Nine receipts from six unique source texts, eight originally accepted and one rejected; nine actual authoritative-verifier witnesses. Independent replay matched retained profile receipts. JSON-only shape mutations are separate controls |
| Scoped CLI completion contract | 130 existing plus seven independent methods and 19 orchestrator invocations with synthetic workers; review manifest `086e8e23c9a46cae396cb1d9cb96cca9e1ab0e20f5fcd3a033a5bdba152d4079`. This validates orchestration and does not execute the final compiler corpus |
| Portable native runner controls | 19 baseline plus nine independent runner controls passed. This historical control snapshot predates the final 20-method runner and real native execution recorded below |
| Actual native source-facade collection | 7,792 ELF process executions, 160 saved LLVM modules and 96 compiled ELF artifacts; 56 unique LLVM hashes and 48 unique ELF hashes. Collection manifest `a461125633a1e06c8f0a83810d2cb5f77232f024a92c0f20a1c8a10a31377aa3` |
| Historical physical store audit | 676 non-fuel data/entry stores, including 94 entry copies; 810 fuel admissions/counter-update guards, 48 overflow guards and 218 prior-overflow dependencies separately audited. Four valid-LLVM negative controls were retained. 676 is not every LLVM store instruction |
| Native semantic matcher | Actual pinned observations and 676 enrichment/layout joins were reviewed; validation of wire domains, consistency, target binding, exact budgets and mandatory artifacts required corrections. Those initial findings were corrected and independently closed; final matcher and rerun identities are recorded below |
| Exact source Batch pilot | Source SHA-256 `dbbadef9a035e3af62aab6ae2ca0ac19531684e4ad8ef25bc4d81a4fa7c07db2`; result 816, 506 charged operations, fuel 1,297, 1,298 budget executions, main X 142, peak X 188 and peak dynamic reference bytes 1,376. Source-free ELF ran with clean environment and no tools on PATH. Pilot manifest `995527e2d5eb9c2e5565c4d7b106ed91dae9b6701cc9bf9159ff1d8b843d196d` |

The initial 429-source model had two fixtures using unsupported standalone blocks.
They were corrected to use existing `if` bodies; the original sources remain
explicit E0100 negatives in the fixture inventory. The 431-source amendment
retains exact diagnostics: among 216 alias-family rejections, 189 have a single
allowed complete tuple and 27 allow two specifically reviewed owner-root tuples.
No broad code-only alternative is admitted. The original mismatches and adapter
failures were retained, including incomplete/duplicate inventories, permissive
nested raw receipts and a committed-store-prefix mismatch mislabeled as MATCH.
These corrections preceded acceptance of their revised evidence scopes.

## Independent source model and diagnostic selection

The independent model lives in `scripts/owned_source_model.py`; its harness and
tests are `scripts/verify_owned_source.py` and `scripts/test_owned_source.py`.
The corpus contract is in
[`tests/fixtures/owned_source/README.md`](../../tests/fixtures/owned_source/README.md).
Generated negative sources stay in frozen evidence directories or JSON fixtures,
not the recursively discovered legacy source corpus.

```sh
python3 -m unittest discover -s scripts -p 'test_owned_source.py' -v
python3 scripts/verify_owned_source.py --mode freeze \
  --evidence target/owned-source-model
python3 scripts/verify_owned_source.py --mode model \
  --frozen target/owned-source-model/frozen \
  --evidence target/owned-source-model-recheck
```

The unchanged frozen corpus retains stable case IDs/seed, original UTF-8 source,
source hashes, half-open byte origins, names/types, scalar results or complete
diagnostic tuples, owner/loan facts and independently derived event schedules.
Its model-only manifest correctly reports zero CLI/native observations; actual
observations are in the separate final CLI report. The bounded model limits are
100,000 dynamic events, 100,000 static states per function and four reference
parameters per function. Exhaustion is `MODEL_INCOMPLETE/error`, never acceptance.
These model limits do not reduce the compiler's advertised source limits.

| Model/review evidence | Result |
| --- | --- |
| Unique sources and outcomes | 431 sources across 17 families: 133 check-accepts, 298 frontend/ownership rejects; exact family counts below |
| Frozen expectations | SHA-256 `297609147384f087a6252645f8f496f01f56132703fdcd45553b1d27b39a3ccc`; final CLI harness verifies the complete unique ordered inventory against the independent model |
| Original mismatch preservation | Original 429-case comparison and standalone-block mistakes retained; corrected sources plus original E0100 negatives form the reviewed 431-case amendment |
| Original standalone-block negatives | Retained in `tests/fixtures/owned_source/historical-standalone-blocks.json` and independently rejected by both final CLIs |
| Exact reference modes | Both mismatch directions reject; explicit `&*p` accepts. Covered by the frozen corpus, final public boundary tests and independent activation scenarios |
| Multi-root diagnostic policy | 189 singleton and 27 specifically reviewed two-root complete-tuple alternatives among 216 alias rejections; no tuple-field mixing or unrelated alternatives |
| Actual selected diagnostics | Both profiles selected exactly the same 216 complete alias tuples; final CLI report binds each code, stage, primary and related byte spans |
| Independent held-out activation | 40 scenarios, 177 candidate checks/profile and 582 total subprocesses, plus six Batch relocation executions; four repository-routing methods. Review SHA-256 `980dc4d38d2f09e82e63330ba7ca8c04588f122530612927e7d6dc5a85ec3ec6` |


| Frozen family | Sources | Accept | Reject |
| --- | --- | --- | --- |
| `00-scalar-contract` | 16 | 4 | 12 |
| `01-records` | 14 | 10 | 4 |
| `02-names` | 10 | 3 | 7 |
| `03-excluded-syntax` | 19 | 0 | 19 |
| `03-excluded-types` | 8 | 0 | 8 |
| `04-moves` | 13 | 4 | 9 |
| `05-condition-grammar` | 4 | 3 | 1 |
| `06-flow` | 14 | 11 | 3 |
| `07-alias-partitions` | 290 | 74 | 216 |
| `07-immediate-arguments` | 4 | 1 | 3 |
| `08-capabilities` | 8 | 5 | 3 |
| `08-integrated-pilot` | 1 | 1 | 0 |
| `08-integrated-pilot-variants` | 7 | 2 | 5 |
| `09-nested-lazy` | 8 | 5 | 3 |
| `10-stores` | 3 | 2 | 1 |
| `11-results` | 7 | 7 | 0 |
| `14-diagnostics` | 5 | 1 | 4 |

The final profiles match the reviewed allowed complete diagnostic tuples,
including code, stage, primary and all secondary origins. Selection is identical
across debug/release.

Newly recognized syntax can intentionally change an earlier unsupported error:
`&bool` parameter syntax moves from E0101/parse at `&` to E0202/resolve at `bool`.
Scalar borrowing remains rejected. Distinguish such documented unsupported-form
diagnostic migrations from regressions in accepted scalar programs or ordinary
scalar diagnostics.

## Source boundary and native checks

These registered Rust checks exercise the source facade, independently of the
public CLI. The same facade was exercised before parser/driver activation;
those historical executions are not final production CLI evidence.

```sh
cargo test --locked --bin oxid owned_syntax
cargo test --release --locked --bin oxid owned_syntax
cargo test --locked --bin oxid frontend::oir::owned::source::
cargo test --release --locked --bin oxid frontend::oir::owned::source::
cargo test --locked --bin oxid frontend::oir::source::
cargo test --release --locked --bin oxid frontend::oir::source::
python3 scripts/verify_owned_witness_privacy.py
```

The currently registered held-out source-native test requires fresh output
directories. It writes real LLVM and ELF artifacts, uses a source-free execution
directory and clean runtime environment, and checks success/failure behavior.
The additional bounded source-native collector and its separate semantic/store
audits are reproduced by the portable command below. A zero-test filtered run,
incomplete case inventory or partial artifact collection is not a pass.

```sh
OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE=target/owned-source-native-debug \
  cargo test --locked --bin oxid \
  frontend::oir::owned::source::reviewer_source::reviewer_source_native_heldout_successes_and_failures \
  -- --ignored --exact --nocapture --test-threads=1
OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE=target/owned-source-native-release \
  cargo test --release --locked --bin oxid \
  frontend::oir::owned::source::reviewer_source::reviewer_source_native_heldout_successes_and_failures \
  -- --ignored --exact --nocapture --test-threads=1
python3 scripts/verify_owned_native_cfg.py target/owned-source-native-debug
python3 scripts/verify_owned_native_cfg.py target/owned-source-native-release
```

| Final source-facade evidence | Result |
| --- | --- |
| Ordinary compiler suites | 700 passed per profile: 566 unit plus 134 integration tests across 16 suites; 21 ignored separately. Final log hashes: debug `69b2fc285703ace91c7caedaaa6220657f51bc5d3afb9aedd0e8132109a2917d`, release `d4288d4504625f1200c67a4ef53cd90d19d97b9d1618e27c496e4b360253fd05` |
| Source/raw declaration, field and parameter correspondence | Final ordinary source tests pass; all 16 native source/profile comparisons pass their independent source/raw fact checks. Broader 431-receipt comparisons remain historical as identified above |
| Raw-valid mistranslation controls | Final ordinary tests pass the mutability/nominal/origin, reference-mode/field-identity and interleaved-parameter permutation controls; actual nine-receipt translation experiment remains separately historical |
| Invalid-raw E0500 and privacy | Final ordinary diagnostic/denial controls pass. The final standalone privacy run passes all 22 probes: four allowed immutable operations and 18 rejected forgery/mutation/consumer-boundary attempts |
| Real native outcomes and artifacts | PASS: 7,792 process observations across eight sources × two profiles. Per profile: 80 LLVM files with 56 distinct hashes and 48 ELF files with 48 distinct hashes; hash sets match across profiles |
| Failure-before-store physical audit | PASS: 676 non-fuel data/entry stores including 94 entry copies, 810 separate fuel guards, 48 overflow guards and 218 prior-overflow dependencies across both profiles; these are site counts, not dynamic writes |
| Physical-audit controls | Five valid-LLVM controls pass: four intended rejections (early store, delayed store, removed admission branch, early overflow-result store) and SSA-renaming acceptance; summary SHA-256 `5d29f360c35f2ac739236b52bf9a31e3fe6fa8819501ce5631e96259f5414a75` |
| Portable runner terminal result | PASS for its bounded native scope; `result.json` SHA-256 `1a9029380690b9567f7a72070d902389cb6a7447f4906a2549fce693db63dd52`; collection manifest `f377dea6064658ff457de203cfdb6ed461488f9c09053721cede0c37afd8b12a`; 34 report hashes verified |
| Explicit evidence-dependent matcher controls | 18 methods pass, zero skipped/errors/failures, including 32 artifact/wire/binding rejection subcases; result SHA-256 `3582fc06d1e5e4280c8a44d6e2b266b5790beb30cc66dd91590e98a1a643a7ff` |
| Final portable pipeline independent review | PASS with no unresolved material finding in its bounded scope; 32,420 retained evidence-file hashes verified, 20 runner methods plus nine independent held-outs and 18 persisted matcher/binding methods pass. Review-manifest SHA-256 `1817b22a478570a5fe39b711ae0c0beeefa7ee9fecf269656d8b51e94b4fb887` |

The final portable review checked 23,776 direct catalogued artifact hashes,
15,584 exact stdout/stderr captures, 7,792 one-ELF hardlink execution directories,
all 256 source-snapshot inputs, both Cargo-discovered test binaries, 14 process/log
manifests, two semantic reports and 16 store/fuel audits. The full 32,420-file
first-run manifest is SHA-256
`cac958009f8b78e3e8488fe795ff33d5b6869062767dc8dc837b39922aa159a6`.
The native source snapshot is
`f8427445114d9417a05da3010d68badd7f1c9ff0629207fc21e2a94bd8aa7b09`;
its compiler manifest is
`aade7bc7df43c345adaab20ab73c2b2968eb60e4c6e8483a2fb671da2803c3e2`.
These scope-specific manifests complement the 240-file public input manifest.

Two earlier runner defects were fixed: omitted nested/dangling symlink inputs
now reject before inventory filtering, including scan-root ancestors; Python
children now use the hashed canonical interpreter rather than an unbound alias.
Controls that failed on the original runner pass on the final version. These
are distinct from the seven earlier native matcher findings concerning physical
metadata, wire domains, contradictory outcomes, execution identity, artifact
inventory, canonical-file binding and exact-byte/newline matching. All seven
matcher findings are independently closed. Original failures and corrections
remain retained; none is treated as a compiler semantic failure.

The portable pipeline reproduces the bounded native collection in both profiles
and retains frozen source expectations, source/build/binary/tool identities,
complete logs, LLVM/ELF artifacts, semantic comparisons and an independent
physical store/guard audit. `--evidence` must name a fresh directory; omitting it
selects a unique directory beneath `target/owned-source-native`.

```sh
python3 scripts/verify_owned_source_native.py --llvm-bin "$OXID_LLVM_BIN" --jobs 2 \
  --evidence target/owned-source-native-check
NATIVE_OBSERVATIONS=target/owned-source-native-check/collection PYTHONPATH=scripts \
  python3 -m unittest -v test_owned_source_native_matcher test_owned_source_native_binding
```

The two explicitly enabled matcher-control modules run 18 methods, including
the 32 preserved artifact/wire/binding rejection subcases. Ordinary Python
discovery without `NATIVE_OBSERVATIONS` skips these evidence-dependent classes;
the native CI job runs them after collecting real observations.

The exact collector is
`frontend::oir::owned::source::program::candidate_adapter::candidate_native::emit_candidate_native_receipts`;
its historical `candidate` name denotes the test-only source-facade collector,
not a separate public command. The runner owns its evidence environment and
checks the exact collector/control registration before execution. Direct
collector runs alone do not establish all of the runner's obligations.

The frozen native corpus has eight sources and 1,932 exhaustive budgets per
profile, including Batch's 1,298. Independent semantic value projection covers
source `Scalar(Store)`, `WriteField` and `Replace` events only. Constructor,
transfer, parameter, snapshot and merge stores are not independently matched as
source semantic values. The physical guard audit separately covers all stores,
including entry copies and fuel updates; pointer stores are checked for occurrence
and control flow, with no prediction of pointer bits. Every physical event is
validated for its wire domain before projection; pointer events use a zero
occurrence marker. Independent LLVM GEP/layout joins bind projected targets.
This is a bounded mapping for the frozen emitter ABI, not a general simulation,
alias-identity or memory-safety proof. Actual reference Charge events supply the
successful charge-prefix comparison; no complete native charge trace is claimed.
Native errors expose code/start line/column, while full spans are separately bound
through source/raw sites and reference outcomes.

Raw verification cannot detect every well-shaped mistranslation of source
mutability, nominal identity, parameter modes/order or snapshot timing. The
separate source/raw correspondence and behavior controls cover those raw-valid
mutants within their recorded scope.
The retained physical audit supplies the separate control-flow/store-dominance
evidence; error-output parity alone does not establish store ordering.

## Production CLI qualification after activation

The exact final debug/release CLI hashes above were exercised after production
parser/driver activation against the unchanged independently frozen corpus. The
reproduction command is:

```sh
cargo build --locked
cargo build --release --locked
python3 scripts/verify_owned_source.py target/debug/oxid target/release/oxid \
  --mode cli --frozen target/owned-source-model/frozen \
  --evidence target/owned-source-evidence --jobs 2
```

The separately reviewed `--mode cli` completion contract writes
`production-cli-result.json` with `status: CLI_ELF_PASS` and returns exit 0 only
for the complete scoped CLI/ELF comparison. Its `full_qualification` remains
`false`: real LLVM/store proof, exact source fuel sweeps, resource boundaries and
independent review are separate obligations. With the current 431-source frozen
inventory, the actual final run completed 2,852 CLI invocations and 264 ELF
executions across debug/release. Its terminal report SHA-256 is
`d0dec6bb99344bc72f2374eb743520339be8170786a29dba1e51bc952935e20d`.
Each profile retains 132 per-case ELF files with 80 distinct ELF SHA-256 values;
all paired ELF bytes match. The report key `unique_elf_artifacts` counts per-case
artifacts, not distinct content. This CLI harness does not retain emitted LLVM
text; no observed LLVM-module count is inferred from its ELF count.

The original/default `--mode production` retains `status: PARTIAL` and exit 2,
including after successful CLI/ELF comparison. That retained behavior makes the
historical preactivation result interpretable; it must not be relabeled success
or suppressed by shell syntax. A `CLI_ELF_PASS` result does not turn any separate
native/resource gate into PASS.

The exercised public Batch command forms are:

```sh
target/debug/oxid check fixtures/owned_source/batch.ox --edition typed-preview
target/debug/oxid run fixtures/owned_source/batch.ox --edition typed-preview
target/debug/oxid compile fixtures/owned_source/batch.ox \
  --edition typed-preview --backend llvm --output target/batch-debug.elf
target/release/oxid check fixtures/owned_source/batch.ox --edition typed-preview
target/release/oxid run fixtures/owned_source/batch.ox --edition typed-preview
target/release/oxid compile fixtures/owned_source/batch.ox \
  --edition typed-preview --backend llvm --output target/batch-release.elf
```

Compile never executes its result. The CLI harness used fresh output destinations,
then executed each ELF in an isolated source-free directory with Oxid/LLVM absent
from runtime PATH, capturing status/stdout/stderr. Batch returned 816 from six
retries, six commits and checksum 210. Its exact source fuel/store obligations
were checked by the separate native pipeline rather than inferred from CLI output.

| Production/source Batch evidence | Debug | Release |
| --- | --- | --- |
| Compiler identity | Debug CLI/test executable hashes above | Release CLI/test executable hashes above |
| Unique source programs / rejects | 431 / 298 | 431 / 298 |
| CLI invocations / native process runs | 1,426 / 132 | 1,426 / 132 |
| Public CLI artifacts | 132 ELF files, 80 distinct ELF hashes; LLVM text not retained | 132 ELF files, same 80 distinct ELF hashes; LLVM text not retained |
| Public Batch ELF SHA-256 | `27da198567855231710fc8c881c3c442225e103e97f668096d61853c6627c1af` | Byte-identical to debug |
| Batch check/run/compile and source-free ELF | All succeed; run/ELF stdout `816\n`, exit 0, empty stderr | Same |
| Separate source-native fuel / charges | 1,297 / 506 charged operations | Same |
| Batch exhaustive budget outcomes | All 1,298 budgets 0..1,297 agree; 1,296 exhausts fuel and 1,297 returns 816 | Same |
| Semantic stores and failure prefixes | MATCH for the documented projection; prior writes retained and unpaid final write absent, with separate physical guard audit | Same |
| Source-derived Batch resource seam | 3 frames, 40 scalar slots on peak path, X 188, dynamic bytes 1,376; 2,200 requested bytes at the lowered three-header reservation | Same; final ordinary source-resource tests pass |
| Scoped harness result | CLI_ELF_PASS; separate native PASS; both retain false full-qualification flags | Same |

The five final `tests/typed_owned_boundary.rs` tests pass in each profile,
including actual file-loader admission at 1 MiB and rejection one byte over.
Compatibility checks in `tests/typed_frontend.rs` and `tests/edition_boundary.rs`
also pass. Independent activation review adds 19 scalar, 13 owned-negative, four
owned-success and four entry-admission scenarios. It performs 114 exact scalar
baseline/candidate comparisons plus 63 owned checks per profile: 177 candidate
checks/profile, 582 subprocesses total including 228 baseline runs.

These checks cover whole-module routing, skipped/unused syntax, precise semantic
stages, entry identity and scalar output, exact borrow modes and absence of
fallback. Early compile failures preserve existing output/cache/OXBC state and
make no native tool call; valid admitted sources reach the tool boundary. Fake
tool markers prove ordering and effects, while the separate real CLI/native
collections establish actual LLVM/ELF behavior.

## Resource evidence and what the bounds mean

The contracts are in [typed preview](../../spec/typed-preview.md#resource-and-trust-bounds)
and [native preview](../../spec/native-preview.md#owned-module-storage-costs-and-private-representation).
The results distinguish inclusive production limits, exact lowered seams and
raw-only bounds. Ignored maximum tests are not hosted coverage unless the workflow
explicitly invokes them. The final ordinary suites pass the source-derived
frame/cell/native-admission and allocation/diagnostic tests; separate ignored
maximum/native execution gates are tracked below.

The currently registered large-source and raw resource commands are:

```sh
for profile in debug release; do
  profile_args=()
  if [ "$profile" = release ]; then profile_args=(--release); fi
  cargo test "${profile_args[@]}" --locked --bin oxid \
    frontend::oir::owned::source::budget_tests::production_token_envelope_lowers_a_large_real_source_graph \
    -- --ignored --exact --nocapture --test-threads=1
  cargo test "${profile_args[@]}" --locked --bin oxid \
    frontend::oir::owned::origin_tests::maximum_block_some_origins_retain_linear_scratch_and_exact_work \
    -- --ignored --exact --nocapture --test-threads=1
  cargo test "${profile_args[@]}" --locked --bin oxid \
    frontend::oir::owned::tests::maximum_block_shape_uses_linear_scratch_and_oversize_product_is_preflighted \
    -- --ignored --exact --nocapture --test-threads=1
done
```

This block uses Bash. Each exact filter must report one executed test. The real
1 MiB source-file-loader boundary must be exercised separately: creating a
SourceMap entry bypasses file loading, and a token/node-envelope test is not a
file-byte-limit test. Comment padding must respect the 65,536-byte token cap.

| Resource obligation | Evidence and measured boundary |
| --- | --- |
| Actual file loader: 1,048,576 bytes and one over | PASS in both final public boundary suites; this reads a real file, not a preconstructed SourceMap |
| Tokens/nodes, depth, arity and declarations | Final ordinary syntax/type/declaration boundary tests pass, including arity 256. Separate ignored 100,000-token maximum passes in both final profiles; exact measured source expansion below |
| Source raw payload, expansion/events/work | Final ordinary exact-count/one-under, checked arithmetic, cleanup preflight and forged-count controls pass; 64 MiB is the source raw requested-payload ceiling |
| Fallible reservations and release parity | Final ordinary output/map allocation-failure sweeps and release-mode count checks pass; no output allocation precedes a preflight rejection |
| Verifier metadata/scratch and origins | Final ordinary tests pass for independent recounting and 32 MiB ledgers. Both ignored maximum-block variants pass in each final profile; these raw-only maxima remain distinct from source admission |
| Producer-map / traversal representation | Conservative producer-map bound 8,800,000 bytes; measured fixed traversal representation 33,872 bytes, separately from machine-stack/RSS; representation is bound by the input manifest and typed specification |
| Owned diagnostic bounds | Final ordinary 64/65-byte names, zero/eight/nine missing names, component caps, exact 2,048-byte retained text and 100/101 diagnostic-stop tests pass |
| Reference runtime resource limits | Final ordinary source tests pass 1,024/1,025 frames, 200,000/200,001 expanded cells and exact lowered byte/cell/frame seams with error priority preserved |
| Native storage admission | Final ordinary source tests pass S+O 256/257, X 8,192/8,193 aggregate admission and lowered path/byte seams; both inclusive source boundary programs compile and execute as source-free ELF in each final profile |
| Native diagnostic/LLVM text caps | Final ordinary acyclic/guarded bounded-emission and count-only rejection tests pass; 16 MiB diagnostic and 64 MiB LLVM text ledgers remain separate |
| Maxima and masked byte ceilings | Source-derived limits and lowered seams are detailed below; raw-only maxima are not relabeled source evidence; byte maxima are masked by earlier cell limits |
| Peak RSS | Retained independent release resource-pair observation: 32,680 KiB including observer/test storage; this predates activation and is not an RSS bound or new final measurement |

### Final explicit source and raw maxima

The separately invoked source maximum passes in both profiles with 259,986
source bytes, exactly 100,000 tokens, 19,996 loops, 59,989 blocks, 19,997 scalar
slots, 24,476,024 requested raw bytes, 1,279,752 producer-map bytes, 159,972 counted
origin work, 4,479,216 origin metadata bytes and fuel 79,988. This exercises the
real source token envelope; it neither reaches the 64 MiB raw payload ceiling nor
substitutes for the separate public file-loader boundary test.

Both raw maximum-block variants pass with 9,900,000 scratch bytes and 16,800,216
metadata bytes. Counted work is 32,400,180 without the Some-origin variant and
33,600,192 with it; the latter reports block/statement/origin-field representation
sizes of 328/208/56 bytes. These raw-only graphs do not fit the source 64 MiB
output envelope and are not claimed as source-level maxima. This final execution
records charged quantities, not a new peak-RSS measurement.

### Retained source-derived resource boundaries

The source-resource review passed ten ordinary tests per profile (eight original
plus two independent controls) and both native inclusive-boundary ELF programs
per profile. Review SHA-256 is
`da98bd8cb031ccfffcbc1552a7f578c8fa91a9b3cc1029fcd2c51d34edbf7ac4`.
All fixtures used real lexing, parsing, resolution, typing, lowering and raw
verification before consumer execution. Their ten ordinary tests pass again in
both final activated suites. Both inclusive source-native ELF cases also pass in the final ignored native
group in each profile, separately from the historical execution identities.

| Boundary | Retained source-derived evidence | Scope |
| --- | --- | --- |
| Reference frames | 1,024 shared-reborrow recursive frames return 7 at fuel 51,195; attempted frame 1,025 gives E0602 at the recursive Invoke. Native rejects the same recursion | Actual production frame limit |
| Reference expanded cells | 200,000 cells with 1,000 frames, 175,998 scalar slots and fuel 384,003 return 7; 200,001 gives E0605 before frame 1,000 is installed, with 999 entries and no returns | Actual production X limit; scalar-slot maximum is not independently reached |
| Native combined slots | 254 scalar plus two owner slots passes S+O 256; one more expression gives 257 and E0700. The inclusive case executes as ELF in both profiles | Actual production limit and inclusive native execution |
| Native aggregate/path storage | 32-function chain reaches aggregate/live-path X 8,192, S 7,469, 63 blocks and 60,504 requested arena bytes; inclusive ELF passes. One more leaf expression gives aggregate 8,193 and E0700 | Actual aggregate limit and inclusive path/depth; direct path rejection uses the lowered 8,191 seam |
| Batch | 3/2 frames, 188/187 cells and 2,200/2,199 bytes separate result 816 from the exact failure at `commit(&mut *state, job)`; bytes are `3*272 + 8 + 1376` | Exact source-derived lowered seams |
| Owner-class runtime accounting | Peak X 50 and dynamic bytes 376; two reserved headers plus result require 928/927 bytes for pass/fail. An independent three-header reservation with only two live frames requires 1,200/1,199 bytes | Lowered seams; reserved capacity is charged |
| Owner-class native storage | Independently counted aggregate 59 cells/80 bytes and path 50 cells/72 bytes; 58/49/79/71 deny the respective boundary | Exact source-derived lowered seams |
| High-owner control | 127 empty-record lets produce 254 owner slots; two/three scalar slots exercise S+O 256/257 | Independent alternate source control |

On the measured target, `Dref <= 8X` and `Dnative <= 8X`. Earlier cell limits
therefore bound reference requests to at most 1,878,536 bytes including 1,024
reserved 272-byte headers and an eight-byte result, and native requests to at
most 65,544 arena bytes including the optional fuel cell. The 16 MiB reference
and 1 MiB native byte ceilings cannot independently be reached under those
limits; lowered seams and this bound are the relevant evidence. Likewise, native
aggregate X 8,192 masks a default path denial at 8,193. Raw-only maxima remain
separate historical evidence.

A fresh independent release execution of the 200,000/200,001 pair measured
32,680 KiB peak RSS, including observer/test storage. Its successful source's
charged peak was 1,878,528 bytes. Neither number is a process-RSS limit. Runtime
fixtures are in `runtime_resource_tests.rs`, native boundaries in
`native_resource_tests.rs`, with the two independent controls in
`reviewer_resource_runtime.rs` and `reviewer_resource_native.rs` under
`src/frontend/oir/owned/source/`.

New owned diagnostic text is at most 2,048 retained bytes per diagnostic
(1,024 message + two 256-byte labels + two 256-byte notes), or 204,800 at the
100-diagnostic ceiling. This excludes preexisting shared lexer/parser formatting,
capacities, headers, allocator overhead, rendered output, escaped paths and RSS.
Do not extend it into a claim about every frontend diagnostic.

Source raw/map ledgers exclude source/AST/typed frontend storage, diagnostic
text/rendering, declaration facade, verifier/plan/consumer storage, allocator
overhead/excess capacity, LLVM processes and total RSS. The measured fixed source
frame representation is distinct from actual machine-stack use. Native explicit
arenas are distinct from LLVM spills/ABI stack and process memory. The raw
300,000-block case exceeds the source 64 MiB output ceiling, so its result is not
source admission evidence.

## Final regression and publication ledger

These commands cover repository checks and scalar predecessors; they do not
replace the source gates above. Preserve terminal results and counts per profile.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
python3 scripts/verify_owned_witness_privacy.py
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/verify_i32_literals.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_suite.py target/debug/oxid target/release/oxid --jobs 2
python3 scripts/verify_feature_status.py
python3 scripts/verify_repo.py target/release/oxid
```

Explicit native gates, in addition to ordinary Rust tests. The
`frontend::oir::owned::native::tests::` ignored group contains ten tests per
profile: nine historical raw tests and the source-derived
`source_resources::source_native_actual_slot_and_cell_boundaries_use_real_llvm`.
The latter compiles and executes both inclusive native source resource fixtures.
The four scalar filters below remain separate.

```sh
OXID_OWNED_NATIVE_EVIDENCE=target/owned-native-debug \
  cargo test --locked --bin oxid frontend::oir::owned::native::tests:: \
  -- --ignored --nocapture --test-threads=1
OXID_OWNED_NATIVE_EVIDENCE=target/owned-native-release \
  cargo test --release --locked --bin oxid frontend::oir::owned::native::tests:: \
  -- --ignored --nocapture --test-threads=1
python3 scripts/verify_owned_native_cfg.py target/owned-native-debug
python3 scripts/verify_owned_native_cfg.py target/owned-native-release
for profile in debug release; do
  profile_args=()
  if [ "$profile" = release ]; then profile_args=(--release); fi
  for test in \
    frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm \
    frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm \
    frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm \
    frontend::oir::loop_control_tests::loop_control_uses_real_llvm; do
    cargo test "${profile_args[@]}" --locked --bin oxid "$test" \
      -- --ignored --exact --nocapture --test-threads=1
  done
done
```

The input-manifest-bound [CI workflow](../../.github/workflows/ci.yml) registers
the four scalar real-LLVM filters and ten-test owned native group per profile,
plus the exact held-out source-native test once per profile with fresh
`OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE` directories. Its source CLI command is
`python3 scripts/verify_owned_source.py target/debug/oxid target/release/oxid --mode cli --jobs 2`.
Its portable command is
`python3 scripts/verify_owned_source_native.py --llvm-bin "$OXID_LLVM_BIN" --jobs 2 --evidence target/owned-source-native-ci`.
The following step sets `NATIVE_OBSERVATIONS=target/owned-source-native-ci/collection`
and `PYTHONPATH=scripts`, then runs
`python3 -m unittest -v test_owned_source_native_matcher test_owned_source_native_binding`.
The unchanged seven-corpus native dispatcher follows. These are verified
registrations; hosted execution is not yet claimed. The token-envelope maximum
remains a separate local gate and is not hosted merely because it is compiled.

| Final gate | Recorded result |
| --- | --- |
| Formatting and strict Clippy | PASS: `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features --locked --offline -- -D warnings` |
| Ordinary Rust debug/release | PASS: 700 per profile (566 unit + 134 integration), 21 ignored, 16 suites; log identities above |
| Ten ignored owned LLVM tests/profile and CFG audit | PASS: nine historical raw tests plus one source boundary test per profile; 198 LLVM files and 198 ELF files per profile (198 distinct LLVM hashes, 197 distinct ELF hashes); the CFG audit excludes the deliberate mutant and covers 197 normal modules |
| Four ignored scalar LLVM tests/profile | PASS: each exact logical/mutable/cyclic/loop-control filter executes one test in each profile, with no skips |
| Held-out source-native and maximum-resource gates | PASS: one held-out source-native test plus three maximum-resource tests per profile; both source-held-out CFG audits pass; portable collection remains separately counted |
| Privacy probes and all Python tests | PASS: 22 privacy probes and 191 Python methods, zero skips; `NATIVE_OBSERVATIONS` enables the 18 evidence-dependent methods within that final discovery run |
| Seven scalar native corpora on final binaries | PASS: 7/7 with jobs 2, bound to both final CLI hashes; suite log SHA-256 `e5b575a1ac14c33d47768e8a7c073c15d4ce56b0298cbfc7f75f92d48a80e244` |
| i32 literal/arithmetic oracles | PASS: literal oracle 1,670 invocations, 320 sources, 502 literal cases and 13 unsupported forms; arithmetic oracle 4,100 invocations, 1,025 sources, 602 successful results and 423 overflow results across both profiles |
| Feature inventory and README structure | PASS for 25 entries with one experimental/production-path ownership capability and matching English/Traditional Chinese/Japanese structure; actual repository verifier passed |
| Repository source/runnable inventory | PASS: 122 sources, 68 runnable programs = 121 legacy sources, 67 legacy runnable programs and one explicitly typed Batch source/run |
| CI commands and source routing review | PASS for configured coverage, including the explicit post-native evidence-dependent control step; not hosted execution |
| Independent activation review | PASS for final source routing, CLI boundary and fixture relocation; bounded matcher and final portable pipeline reviews also PASS |
| Hosted CI on exact publication head | Not yet run; local results do not establish exact-head hosted status |

The final gate command manifest contains 31 completed commands, every exit code
zero and every expected Rust test count satisfied; SHA-256
`defa7af2cb65d073282075e0f2912e797decd4521e4652759f1781e76129281b`.
It binds the commands, elapsed times and complete log hashes. The two ordinary
suite runs and complete portable native run are separately bound above.

The 21 tests ignored by each ordinary Rust run are accounted for explicitly:
18 run in the final explicit chain (four scalar LLVM, ten owned LLVM, one held-out
source-native and three maxima); one is the separately completed portable native
collector. The other two source/mutation receipt collectors retain their reviewed
preactivation experiments and were not recollected by the final chain. Their
historical 431-source and nine-translation receipts are not presented as new
final CLI observations.

For each profile, the existing owned CFG audit validates 197 normal LLVM modules,
538 guarded functions, 2,290 named non-fuel stores, 173 overflow-result stores and
3,874 error sinks. The saved collection also contains one deliberate misordered
store mutant, yielding 198 LLVM/ELF files. The separate held-out source audit
covers six LLVM modules, eight guarded functions, 42 named stores, one overflow
store and 79 error sinks per profile. These existing audits cover their named
store patterns; their totals are distinct from the portable source audit's
complete physical store/entry/fuel accounting.

Final privacy/Python log SHA-256 values are
`a0791a8eb4e0b364beb25eae06419ca36d4aab61db763454688c22430f5f934f` and
`d17f18f53aa9c53d1daa10005a9a06c7fe4429deaab4bcec7622ee076b39ac90`.
The final repository rerun again reports 122 sources and 68 runnable programs,
with log SHA-256 `06125e0e65604e31d39e1c619ab067b2177f6a8900867ed776d3bfb1e2600fb8`.

The unchanged scalar native dispatcher passed native preview, checked i32
arithmetic, scalar comparisons, boolean logic, mutable locals, while loops and
loop control. The suite retains its own per-corpus inventories and artifact
manifests; its compiled-artifact counts are separate from the ownership corpus.

| Scalar regression corpus | Reported source cases | Main compiled artifacts | Separate resource artifacts |
| --- | --- | --- | --- |
| Checked i32 arithmetic | 1,051 | 2,102 | Not separately reported |
| Scalar comparisons | 1,610 | 3,220 | Not separately reported |
| Boolean logic | 525 | 1,050 | 14 |
| Mutable locals | 163 | 326 | 10 |
| While loops | 144 | 288 | 8 |
| Loop control | 186 | 372 | 8 |

Native preview separately reports 57 real compiled cases, reference/Python
parity, 32-frame/64-argument execution under a 1 MiB stack, ELF/PIE/libc checks,
output failures and reproducibility. No source/content-uniqueness total is
inferred from that suite's differently scoped count.

The first full repository gate failed because placing the typed Batch under
`tests/fixtures/owned_source/` made the unchanged legacy `oxid test` scanner
execute it as legacy source. The original failure log is retained; its SHA-256
is `734b93aa67dc505bd49b02401cd77c9b87a21ce7284d7fc7fc64db2bc79e946b`.
The exact source bytes were moved to `fixtures/owned_source/batch.ox`; the legacy
scanner was not changed. Six independent actual CLI relocation controls pass
across both profiles, plus four isolated routing methods. The corrected full
repository run passes with 122 sources and 68 runnable programs; its log SHA-256
is `06125e0e65604e31d39e1c619ab067b2177f6a8900867ed776d3bfb1e2600fb8`.
Encoded model negatives remain outside recursively discovered `.ox` suites.

The activation review retains two initial test-harness expectation mistakes:
unit-result JSON has no value key, and scalar field projection reports E0305.
Both expectations were corrected against the documented contract; neither was a
compiler defect. Historical model, adapter and native validator failures remain
identified above. Process counts, saved files and distinct content hashes are
reported separately throughout this ledger. A reused ELF in a budget sweep is
not an additional compiled program. Hosted CI remains a separate exact-head
publication obligation.
