# Fallible lexer tape: current-source authority and direct CI consumer audit

Status: read-only source/qualification design audit. No build, compiler, qualification runner, successor generator, publication, historical checkout, or private-parser research was invoked. This report is not a qualification pass or permission to generate source authority. The source-qualification design needs independent approval before any successor generation.

## Audited checkpoint

- Checkout: `/workspace/scratch/8266abf56995/oxid-lexer-reservations`.
- At inspection, HEAD was `4fb82aca1e0dd6dead3c993dc05a67deaf879d09`, after documentation-only approvals `255b3cd` and `4fb82ac`.
- Merged compiler baseline: `25c5b239`, tree `c79dd3d9458e0c66e531528ad8469c60d21b2948`.
- Immutable current compiler source manifest: `tests/fixtures/typed_project_source_binding/current-source.json`, 74,192 bytes, SHA-256 `402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa`.
- That manifest describes reviewed source commit `e3c1b4a1a3ef457326f11a802896c125202fe797`, tree `5fdb4f06a8fcbc61724676df55a4c3bee130eaf7`. The documentation/merge HEAD is distinct from this selected compiler checkpoint. Do not relabel any older receipt.
- Exact selected source membership: 376 inputs, all 288 `src/` and `native/` members, 291 compiler/build bodies including Cargo.toml/Cargo.lock/build.rs, 78 compile-time fixture bodies, and 136 include expressions. This includes non-Rust proof files under compiler directories.
- Current Unit4 public maps: 376 ordinary / 377 lifecycle-observer members. Current Unit4 parser maps: 539 base / 542 observer-derived / 542 control-derived members; 340 source-delta rows. Unit4 CI input closure: 271 files across eight closed roots. These inventories serve different roles and must not be interchanged.

The SHA and manifest size above were recomputed by this audit. The following scoped source identities were compared with the manifest before production edits began:

- `src/frontend/lexer.rs`: 8931 bytes; `1c282ff02383bf390105c7f6bcf5d7f58cc769c571ed3b8ca1bfc217740971b8`.
- `src/frontend/project.rs`: 54990 bytes; `3839621918c23eb26fa40ffb5fb670941aae529eb31af6afe7438c373eb58151`.
- `src/frontend/format.rs`: 26930 bytes; `c19ac24e435a644c0108d7557700160e5e09a6cda205f4ea6ac35d7adc3ab880`.
- `src/frontend/lexical_provider.rs`: 23720 bytes; `07fc4a9bff1daa48b2d7d720a96dabacbd9a8b1a4fb7b13f18a1d2178d8ae403`.
- `src/frontend/project/budget.rs`: 8651 bytes; `6485f8840c21b7e753d119f9187804ed2588dc84893531f6e4c09042b13b2129`.
- `src/frontend/project/budget_real_null_observer.rs`: 34754 bytes; `7787b1ec84994d6743bdbf2216b792d6889e183f737e2480523ce12972b30c94`.
- `src/frontend/oir/owned/source/array_pipeline.rs`: 29506 bytes; `113073eb50d23589199db70d9ae9a31ef95c57814cb8cac935c914d05453d726`.

## Findings requiring explicit qualification design

1. **Unit1 has a real newly changed failure domain.** `tests/fixtures/typed_project_unit1_independent/reviewer_additional.rs:181–199`, `heldout_checked_counter_overflow_stays_distinct_from_allocation_failure`, dynamically sweeps every shared-allocator request. It assigns only `parse` (module declarations/items) or `source-project` stages and requires text containing `overflow`. Newly reached lexer request ordinals must instead produce the approved `E0400 / lex / token storage resource limit exceeded`. Therefore unchanged current-source execution of this frozen test is expected to refuse the new lexer ordinals. This is a specific compatibility boundary, not grounds to broaden the frozen test or weaken its assertions.
2. **Unit4 public lifecycle instrumentation is attached to the old lexer body.** `tests/qualification/unit4_public_v3/observer-u8-v1.patch:30–45` inserts `lex_attempt` in `lex_with_limit` and `lex_complete` after its EOF append. Moving actual recognition into an accounted core will invalidate patch context or bypass hooks when project/formatter calls the core directly. Keeping the old wrapper signature alone does not preserve lifecycle evidence.
3. **Unit4 parser token instrumentation depends on exact append spellings.** `tests/qualification/unit4_parser_current/portable.py:2198–2219`, `compose_division_lexer`, transforms literal `tokens.push(Token { kind, span });` and `    Ok(tokens)\n` to append one token event for ordinary tokens and one for EOF. A common fallible append helper removes these exact seams. The current composer needs an explicit successor; a source SHA update alone is insufficient.
4. **The separately gated real-null observer is source authority even though test-only.** Both `project/budget_real_null_observer.rs` and its test-only wiring in `project/budget.rs` are in the exact closure. Changes there require the same complete source transition and control/observer maps as production changes. It must not be omitted from a release-only source inventory.
5. **Narrow standalone observers already carry the production allocator module.** Typed and streaming lexer builders copy `lexer.rs`, `diagnostic.rs`, `source.rs`, `project/budget.rs`. Parser/static builders add project and parser dependencies. A fallible wrapper using the existing Allocator can stay in these approved dependency-light closures. Adding any new lexer dependency or unconditional observer dependency requires explicit closure review; do not silently expand copied source or edit frozen wrappers.
6. **No current source seal can be updated in isolation.** Direct CI commands pin the seal themselves, pass it on the command line, or authenticate the whole source-binding package/current-map closure. See the pin inventory and direct workflow inventory below.

## Proposed minimal public adapter strategy (design only; not generated)

Use a separately named outer lexer-reservation current-source transition and separately named qualification adapters. Names such as `lexer-reservation-source.json`, `lexer-reservation-authority.json`, `lexer-reservation-transition.patch`, and `lexer_reservation.py` are illustrative, not produced artifacts. Preserve the complete 402db5 manifest under an explicit byte-storage predecessor name and preserve the byte-storage helper, authority, transition patch, current parser/public authority copies, original Unit1 fixture bytes, all earlier authorities, and all earlier observations byte-for-byte. Current dispatchers may gain an explicit outer stage only after review; a global replacement of old hashes is not an acceptable successor.

### A. Exact outer source transition

1. Freeze the final reviewed compiler checkpoint/tree and final adapter checkpoint separately. Derive the changed-path roster from committed bytes relative to the admitted predecessor, never from an assumed four-file list. Explicitly include test-only observer wiring, its source file, `array_pipeline.rs`, and any added test modules/proof files if they actually changed.
2. Retain all 376 predecessor selected entries. Add only a specifically named, reviewed roster of necessary compiler/test/fixture members. Record removals explicitly; none are currently proposed. Check the actual complete filesystem membership of both `src/` and `native/`, all modes, regular-file/symlink rules, bytes, SHA-256, and Git blob identities. No generic cache/source exclusions.
3. Freeze an exact ordered binary patch with the existing recipe `git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS`. Bind base/checkpoint commits and trees, patch bytes/digest, ordered changed paths, before/after identities, additions/removals, and complete current/predecessor maps in the authority.
4. Independently enumerate every selected `include_str!`/`include_bytes!` expression with exact source, ordinal, and expression. `u8_cross_host.py:include_directives` is a lexical expression inventory, not a general Rust macro evaluator; retain historical macro-generated fixture-closure validation after inversion. Baseline has 136 expressions and 78 fixture bodies. Preserve all old expressions/fixtures exactly, or require explicit named additions and their identities; do not infer closure from Rust filenames alone.
5. Invert only the new outer patch with exact hunk offsets and contexts. Verify complete byte-for-byte recovery of all 376 original selected inputs and their fixture/include closure. Materialize that closed predecessor separately and call the unchanged byte-storage admission helper on it with the original 402db5 manifest. Never point an immutable predecessor helper at new candidate bodies.
6. Continue the existing byte-storage → u8-cross-host → u8 → cache-admission → cache-preservation → package-integrity → lexical-provider → producer-diagnostic → frontend-v2 → HIR producer/import → native inventory/storage → stdout/stdin → enum/projected/unary/composition/slices/division/combined/formatter → original inverse chain unchanged. The final archived selected view remains the original 117 members.
7. Keep execution and source-only claims separate. `preflight`, `prepare-archived`, and `run-unit2 --prepare-only` do not prove semantics. A new candidate must actually be executed by current gates; historical materialization is only for authenticating predecessors.

### B. Unit1 current-only failure-domain adapter

- Preserve `typed_project_unit1_independent` source, rosters, fixture expectations, and original 69-case lineage byte-for-byte. Run unchanged semantic/resource cases against the current compiler; retain historical execution separately where needed.
- Use a named current adapter with exact original-body hashes and an exact reversible modification limited to the new lexer allocation/counter-overflow branch. Do not normalize any ordinary diagnostic or replace broad assertions with acceptance of arbitrary `lex` errors.
- Independently freeze the new allocator site label, kind/ordinal schedule, pending-token span, EOF span, and exact approved diagnostic. Only a request proven to be a lexer tape request may use the new lex-stage/resource-limit outcome. Every preexisting nonlexer request must continue to satisfy the old stage, text, primary-origin, and prefix rules.
- Preserve every original row/case identity in a correspondence receipt; distinguish unchanged executions from the one explicitly adapted failure domain. Add independent exact controls for lexer requests, not a blanket waiver of the frozen counter test. Retain the original test source and both before/after hashes plus inverse proof.
- `heldout_every_controlled_reserve_failure_has_valid_origin` (lines 106–119) already discovers requests dynamically and requires one failed trace, E0400, allocation-failed text, and valid origins. It may naturally cover lexer capacity-failure requests, but its pass is not a substitute for the separately approved inline-failure, no-subsequent-work, growth-null, and exact full-span controls.
- Unit2's immutable u8 accounting adapter currently depends only on declaration_index/resource.rs, declaration_index/sealed.rs, and declaration_index/u8_reservation.rs. These are outside the proposed lexer slice. Prove those three dependencies remain byte-identical across old/new maps; preserve the original accounting authority and transport receipt. Do not silently recalculate a resource endpoint. Any actually affected current endpoint requires a separate independently derived adapter.

### C. Unit4 public lifecycle adapter

- Retain `observer-u8-v1.patch` and all prior inverse patches unchanged. Add a separately named current lifecycle patch/adapter whose reverse reconstructs the exact new uninstrumented source; independently prove its relationship to the retained predecessor hook semantics.
- Place one attempt event at the actual accounted core lex invocation before its recognition/storage work, and one completion event only after successful EOF append, exactly as the old successful/error behavior requires. Do not place a second event in delegating convenience wrappers.
- Explicitly cover ordinary project, provider canonical, formatter first parse and candidate reparse through both syntax routes, dependency-light wrapper calls, and test-only source observer calls. Each actual core invocation gets exactly one attempt; each successful invocation gets one completion; failed invocations get no successful completion. Multiple real invocations, such as formatter reparses, remain distinct. Provider observation is not another canonical core invocation.
- Invalid provider association still follows the approved observation/comparison/canonical schedule. Do not erase receipts for canonical execution already performed merely because the later association check refuses it.
- Require exact path scope, inserted-event scope, event ordering, old/new file maps, inverse proof, missing/duplicate-hook controls, and unchanged public lifecycle rosters. Source identity preservation alone cannot establish hook cardinality.

### D. Unit4 public current-parser token-hook adapter

- Keep the frozen helper and historical parser package untouched. Keep `compose_division_lexer` available for its exact historical source/instrumentation checks.
- Add a narrowly named public current lexer-instrumentation adapter that admits the exact fallible lexer body and inserts the same retained token observation exactly once after every successful append, ordinary and EOF. With a single append helper, one post-push hook can cover both sites, provided the independent proof freezes its call schedule and skips all failures/nonappends.
- Bind exact transformation contexts, source and derived identities, old/new hook roster, and reversibility. For every successful unchanged corpus case, its token-event stream must remain byte-identical. New storage failures must not create a token event for the rejected append or a later parser event.
- Update the complete current parser base, observer-derived, control-derived maps and candidate manifest; keep production parser source, accepted language, historical closed policy, semantic amendments, and fixed observation rosters unchanged. Preserve the current 539/542/542 authority as the named predecessor; actual successor sizes must be derived rather than assumed unchanged.
- This audit inspected only the public current composer and its declared frozen-helper interface; it did not read or execute the frozen private helper or open paused research. If the narrowly scoped successor cannot be specified/qualified with allowed public adapter material and approved retained interfaces, stop at that exact boundary and ask the parent for an allowed alternative. Do not regenerate private parser artifacts or waive its gate.

### E. Negative controls required before source authority acceptance

Exact forward/inverse recovery; every changed hunk/context; changed/missing/extra/new source members; all selected fixture bodies; changed/missing/reordered/duplicated include expressions; modes/symlinks; unordered/duplicated patch sections; wrong-stage and double inversion; coherent helper/manifest/authority tampering; unexpected output/materialization or compiler invocation before admission; complete current and reconstructed predecessor identities; accounting dependency transport; stale observer/control maps; missing/double lifecycle or token callbacks; preexisting fresh-storage observer refusal matrix; and exact current/head receipt association.

## Direct workflow consumer families and consequences

The appendix inventories every direct Python/script, Cargo, and container command from `.github/workflows/ci.yml`, grouped by its actual job/step and original line number. The families below distinguish why they matter; every applicable final-head CI job still has to pass.

### Full repository verification (`quality-and-repository`, lines 20–153)

- `unit2_u8_current/test_current.py`, source-binding Python controls, and `typed_project_source_binding/run.py run-unit2`: exact current admission, full inverse closure, current Unit2 semantics/resources, unchanged accounting dependencies. Unit2 execution expects both profiles, 3,603 semantic cases and 21 resource tests per profile; source-only preparation cannot stand in for those.
- Complete Python `test_*.py` discovery includes source/controller mutations and the standalone observer compatibility controls even where a script is not directly named elsewhere.
- Unit4 current parser `test_current.py`, `test_stdin_initializer.py`, `test_record_composition_amendment.py`, and `unit4_ci/test_integration.py`: complete source maps, prescribed mutations, frozen initializer/amendment behavior. New lexer instrumentation is an additional explicitly reviewed current adapter, not a change to the earlier parser amendments.
- `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-targets --all-features --locked`: test-only growth observer is compiled here, including existing fresh-storage controls and all code sharing the global test allocator.
- `verify_owned_witness_privacy.py`: genuine sibling compile probes against the whole current frontend. `typed_project_unit2_seal/run.py`: actual current frontend private source/index sibling probes. Neither is paused private-parser research; their names refer to Rust visibility. Do not weaken them or claim they execute release semantic behavior.
- `typed_project_unit1_independent/run.py --profile both`: current project loader/source identity/resource controls; see the newly affected frozen overflow branch above. `typed_project_unit1_compatibility/run.py --original-only`: unchanged 31 original public cases.
- Exact i32 literal/arithmetic Python oracles, `verify_repo.py`, optimized runtime build, source installation, and container build/smoke: ordinary lexer/format/module routes and packaging. These do not supply per-site allocation evidence but can reveal no-failure regressions.

### Four-host runtime and artifact join (`cross-platform`, lines 155–238; `compiler-artifact-equivalence`, 240–251)

- Linux x86_64, Windows x86_64, macOS x86_64, macOS arm64: ordinary release build, all-target tests, exact typed formatter goldens, bounded parser/static controls, language tests, concise example, repository build, bootstrap/check, benchmark/report. New cfg(test) source must compile on all four actual hosts; host-specific filesystem refusals remain unchanged.
- Serialized compiler artifacts are compared across all four actual hosts. A local host simulation cannot establish this join.
- `workflow-lint` validates CI and metadata; it is not a lexer semantic consumer, but remains applicable if workflow/source pin adapters change.

### LLVM native preview (`native-scalar-preview`, lines 264–928)

- Archived Unit3 source/native/mutation recipe first calls source-binding `prepare-archived`, then its pinned package, transport, comparison, bridge, assembly, builds, plans, collect, native runtime stage, and mutation controls. It authenticates the new source through the outer inverse but executes archived selected bodies for its historical purpose. Do not relabel that as current lexer qualification. Evidence collectors/preservation and upload steps remain mandatory terminal bookkeeping.
- Direct current source/native bridges: typed_native; raw bool/mutable/cyclic/loop gates; owned source-array LLVM controls; checked-HIR public and private native tests; public fixed-array CLI; producer/independent Unit2D (`verify_owned_array_native.py run/independent/package`); held-out source-native; division, negation, slices, record composition, inventory, projected-slice suites.
- `verify_owned_array_native.py` captures current-source manifest plus source-binding/package/repository input closure, checks preflight and exact HEAD/tree/event source, and packages original receipts. Its source inventory must consume the explicitly admitted new stage, not an arbitrary caller subset.
- Exact bounded enum, u8, byte-storage, stdin, stdout controllers share current sealed build receipts. Bounded enum pins 402db5 directly; stdin/stdout pin it internally and receive it as a literal workflow argument; byte storage pins it directly. U8 consumes enum/stdin build authority and keeps exact frozen test selection. All need current source dispatch review and fresh executed receipts, retaining their prior resource/language/oracle claims and old fixture bytes.
- Byte storage additionally performs `--prepare-public-build`, tests closed oracle/provider admission in normal and optimized Python, and runs source/privacy/resource/native evidence. Existing source privacy controller `verify_byte_storage_source_privacy.py` is a transitive required consumer.
- Expression/parser/evaluator, stdin, stack, typed lexer, typed parser, scalar static frontend, production owned-source CLI, owned-source native budgets/matcher, and full native suite are directly invoked. Their frontend inputs pass through lexer/project/format; preserve all frozen accepted/refused domains.
- Typed lexer: `build_typed_lexer_observer.py` → `verify_bounded_typed_lexer.py`, plus explicit formatter checks on eight `.ox` source files.
- Typed parser/static: `build_typed_parser_observer.py`, `build_typed_static_observer.py`, observer selftests and `verify_bounded_typed_parser.py` / `verify_bounded_typed_static.py`, both profiles, native. They use `legacy_scalar_observer_u8.py` to adapt immutable wrappers, and the canonical lexer wrapper must stay callable. These bounded public source observers are distinct from paused private-parser research.

### Unit4 exact-source four-host gates (`unit4-linux`, `unit4-portable-hosts`, `unit4-qualification`, lines 930–1163)

- Linux workflow fetches exact historical authority commit `d9e6b9bf172abd5e15da7212c9e6224e29ccc768` and supplies it to `unit4_ci/gate.py host --historical-repo ...`, along with actual PR head/event SHA, qualified toolchain/cache, and Linux host. This audit did not fetch/open that checkout or execute this branch.
- Portable hosts restore a fresh committed-byte checkout, run integration controls, and execute ordinary/lifecycle/no-native trap gates on each actual host. Windows retains its native MSVC setup.
- `unit4_ci/common.py`, `gate.py`, `evidence.py`, `join.py` authenticate current seal, input-closure hash, source/public/parser maps, full host/profile rosters, raw streams, complete/incomplete terminal evidence, and exact PR head separately from event SHA.
- Unit4 public `run.py`, `build.py`, `selftest.py`, parser `portable.py`, hosted capability `selftest.py`, and current parser authority controls are transitive consumers. Preserve public freeze/rosters, hosted publication authority, historical parser authority, semantic amendments, and unchanged observer hook meanings.
- Exact scope boundary: if executing the mandatory historical-parser branch is disallowed by the paused private-parser instruction, the branch is BLOCKED pending an allowed method. It must neither run implicitly under a broad CI replay nor be silently omitted. Public controller design can proceed; private research stays paused. Parent must resolve this before claiming complete local qualification. Genuine authorized hosted CI of existing public gates is a separate decision and cannot be forged locally.
- Genuine dpkg-based LLVM staging, root/capability checks, and the mandatory four-actual-host join cannot be replaced by extracted binaries, synthetic receipts, predecessor CI, or local host labels.

### Local HIR producer and streaming lexical gates (lines 1165–1272)

- `run_hir_producer_ci.py`: exact-head ordinary debug/release compiler builds, v1/v2/diagnostic/edge controls, source preservation and executable identities. Direct admission tests run in both normal and optimized Python where prescribed. Frozen producer source manifests, wire schemas and caps remain unchanged.
- `run_streaming_lexer_ci.py`: exact-head ordinary debug/release compilers, `qualify_streaming_lexer.py`, failure controls, and independently built full-source lexer observer. It records ten controller/source-manifest inputs and rebuilds producer closures using selected lexical tokens. `build_streaming_lexer_observer.py` is a required transitive consumer, sharing the four canonical files with the typed lexer observer.
- Existing scripts require Rust 1.99.0 and LLVM 19.1.7 for these qualified jobs, and reject unauthorized profile overrides. Receipt source preservation compares initial and final exact head/tree. Reusing a mutable worktree while another worker edits it will invalidate evidence; final qualification needs an isolated frozen checkpoint and coordinated serial build resources.

## Direct and transitive current-seal pin inventory

Literal 402db5 pins were found in:

- `.github/workflows/ci.yml` (stdin and stdout command arguments).
- `scripts/verify_bounded_byte_storage.py` (`SOURCE_SHA256`).
- `scripts/verify_bounded_enum_native.py` (`REVIEWED_SOURCE_SHA256`) and `scripts/test_bounded_enum_native.py`.
- `scripts/verify_bounded_stdin_native.py` (source SHA and member count), `scripts/verify_bounded_stdout_native.py`.
- `tests/fixtures/typed_project_source_binding/run.py`, `byte_storage.py`, `package-manifest.json`, `byte-storage-authority.json`, and `current-source.json`'s body identity.
- `tests/qualification/unit4_ci/common.py` and `inputs.json`.
- `tests/qualification/unit4_parser_current/authority.json` (with its digest pinned by `portable.py`).
- `tests/qualification/unit4_public_v3/authority.py` (current files, observer files, lifecycle patch and member counts).
- Current documentation in bounded-byte-storage validation and the source/Unit4/byte qualification READMEs.

This is a pin inventory, not an instruction to rewrite these originals. A separately named active successor must preserve predecessor bodies and authenticate all new dispatchers/maps. The source-binding package manifest and Unit4 CI input manifest also pin adapter/helper bytes, so updating one literal without its exact reviewed closure will fail admission.

Other important affected/reachable scripts, without a direct literal 402db5 pin, include:

- `scripts/build_typed_lexer_observer.py`, `build_streaming_lexer_observer.py`, `build_typed_parser_observer.py`, `build_typed_static_observer.py`, `legacy_scalar_observer_u8.py`.
- `scripts/run_streaming_lexer_ci.py`, `qualify_streaming_lexer.py`, `qualify_streaming_lexer_controls.py`, `streaming_lexer_protocol.py`, `build_streaming_lexer.py`, `build_hir_producers_v2.py`.
- `scripts/run_hir_producer_ci.py`, `qualify_hir_producers.py`, `qualify_hir_producers_v2.py`, `qualify_hir_producer_diagnostics.py`, `qualify_hir_u8_compatibility.py`, `qualify_hir_byte_storage_compatibility.py`.
- `scripts/verify_owned_array_native.py`, `verify_bounded_u8_native.py`, `run_bounded_u8_corpus.py`, `verify_byte_storage_source_privacy.py`, and the corresponding admission tests.
- `scripts/verify_owned_witness_privacy.py`, `verify_typed_formatter.py`, `verify_owned_source.py`, `verify_owned_source_native.py`, `verify_native_suite.py`, `verify_checked_hir_import_native.py`, `verify_repo.py`, `verify_fixture_data.py` and source-boundary tests.
- `scripts/test_typed_project_source_binding.py`, `test_stdin_historical_adapters.py`, `test_record_composition_runtime_view.py`, `test_record_composition_observer_adapters.py`, `test_record_composition_current_contracts.py` and `tests/qualification/projected_slice_observer_v1/adapter.py`.
- Unit1 original/public compatibility and Unit2 source/index runners; Unit4 public/current-parser/hosted/CI adapters, selftests and source maps; stdin/stdout current controller tests.

## Separately gated cfg(test) growth-observer consumer fence

The existing observer is pulled in by `project/budget.rs:5–7` only under cfg(test). Its selected initial reserve enters `enter_exact` at vector/string gates. The single test global allocator lives in `src/frontend/oir/owned/reviewer_allocator.rs` and routes actual Alloc and Realloc events; preserve that ownership and avoid a second global allocator.

Existing observer consumers beyond its own controls include `oir/source/hir_import/verify_tests.rs` and `hir_import/candidate/cleanup_controls.rs`. They use the current fresh-storage `with_selected`/`Target` semantics. A growth API must not alter their selection/eligibility/refusal results, nested-arm behavior, overflow behavior, cleanup, layout accounting, or no-call results.

Current observer tests named `real_null_*` cover layout, setup refusals, gate refusals, pre-gate overflow, missing/nested/terminal scopes, private guard no-call, nested closed drivers, selected unwind cleanup, and non-Send/non-Sync guards. Run them unchanged plus the separately reviewed growth controls in both relevant test profiles. All-target Rust builds, Unit1 injected-source tests, Unit2/4 complete current test trees, and exact-source source/privacy native controllers include this source even if a particular selected test does not execute it.

Standalone observer builders invoke rustc without cfg(test) and do not copy `budget_real_null_observer.rs`; this is correct only while all references remain test-only. A production dependency on the growth observer is out of scope and would break their minimal closure.

Required growth-only design evidence remains separate: exact identity/kind/ordinal/old pointer/old len/capacity/Layout/new Layout/operation binding; actual selected null return; old allocation remains owned and valid for one normal destruction; mismatch/nested/missing-call/trace-interference controls; bounded prepaid trace storage; unchanged fresh-storage refusal matrix; no simulated realloc ownership transfer/retry. Later-growth capacity-overflow injection is not actual null coverage.

## Qualification ordering and no-execution claims

1. Freeze the source-qualification design above, including exact Unit1 adapted failure domain and Unit4 lifecycle/token-hook semantics; obtain independent approval.
2. After source implementation and independent semantic/resource review, freeze a compiler checkpoint; then generate only the reviewed named successor and current public adapters. This audit does not generate them.
3. Run source-only admission controls first, with `PYTHONDONTWRITEBYTECODE=1` and `python3 -B`. Exact source/fixture closures reject cache contamination; even empty `__pycache__` directories can matter. Never “solve” failure with broad exclusions.
4. Run repository fmt/clippy, all-target/all-feature locked Rust tests and Python discovery, narrow observer builds/tests, Unit1/Unit2 current public/source gates, provider/formatter/module/source observations, privacy probes, release/verify_repo, and current bounded native controllers at the final frozen source. Preserve failed attempts and explicit skipped/blocked statuses.
5. Run all applicable existing direct workflow commands on the exact final PR head under their prescribed tools/profiles/environments. The appendix is the source of exact baseline commands; illustrative `$RUNNER_TEMP` locations must be fresh, outside the source checkout where required. Shared build resources must be serialized.
6. Private-parser research remains paused. The Unit4 historical-parser requirement is an identified boundary to resolve explicitly, not a waived gate. Actual foreign-host and genuine installed LLVM staging remain hosted requirements.
7. No new process-wide OOM delivery claim follows from passing these gates. Diagnostic allocation, native graph/call recovery, and original RFC0031 compound obligations stay limited as the approved proposal says.

## Appendix: all direct workflow command consumers

The following list is mechanically extracted from the audited `.github/workflows/ci.yml`. It includes every line invoking a Python file/discovery, Cargo build/test/run/install/fmt/clippy command, or Docker build/run, with continued argument lines collapsed. Script paths appearing as arguments (observer/comparison wrappers) are also retained. They are inventory, not executed commands. Unit3 variable-expanded paths refer to `tests/fixtures/typed_project_unit3_independent`; `$unit3_repo` is the checkout. Upload-only/setup-only steps are not source consumers and are summarized above rather than duplicated here.


### quality-and-repository: Test current u8 Unit2 accounting successor controls

- L41: `python3 -B tests/qualification/unit2_u8_current/test_current.py`

### quality-and-repository: Test current and archived source-binding admission controls

- L44: `python3 -B -m unittest discover -s scripts -p test_typed_project_source_binding.py -v`

### quality-and-repository: Preserve Unit2 semantic and resource gates with explicit current inputs

- L49: `python3 -B tests/fixtures/typed_project_source_binding/run.py run-unit2 --repo . --cargo "$(rustup which cargo)" --rustc "$(rustup which rustc)" --output "$RUNNER_TEMP/typed-project-unit2-results"`

### quality-and-repository: Test feature status metadata validation

- L55: `python3 -B -m unittest discover -s scripts -p 'test_*.py' -v`

### quality-and-repository: Test current parser authority and prescribed CI mutation controls

- L59: `python3 -B tests/qualification/unit4_parser_current/test_current.py`
- L60: `python3 -B tests/qualification/unit4_parser_current/test_stdin_initializer.py`
- L61: `python3 -B tests/qualification/unit4_parser_current/test_record_composition_amendment.py`
- L62: `python3 -B tests/qualification/unit4_ci/test_integration.py`

### quality-and-repository: Check formatting

- L65: `cargo fmt --all -- --check`

### quality-and-repository: Run Clippy with warnings denied

- L68: `cargo clippy --all-targets --all-features -- -D warnings`

### quality-and-repository: Run Rust and native bridge tests

- L71: `cargo test --all-targets --all-features --locked`

### quality-and-repository: Check private ownership witness construction and immutability

- L74: `python3 scripts/verify_owned_witness_privacy.py`

### quality-and-repository: Verify private typed source-set identity and resource regressions

- L77: `python3 -B tests/fixtures/typed_project_unit1_independent/run.py --repo . --profile both --output "$RUNNER_TEMP/typed-project-unit1-results"`

### quality-and-repository: Build optimized standalone runtime

- L80: `cargo build --release --locked`

### quality-and-repository: Preserve the 31 original public single-file cases

- L83: `python3 -B tests/fixtures/typed_project_unit1_compatibility/run.py --debug target/debug/oxid --release target/release/oxid --original-only --evidence-root "$RUNNER_TEMP"`

### quality-and-repository: Test typed-project runner identity controls

- L86: `python3 -B -m unittest discover -s tests/fixtures/typed_project_unit2_independent -p 'test_protocol.py' -v`

### quality-and-repository: Check private typed-project source and index seals

- L89: `python3 -B tests/fixtures/typed_project_unit2_seal/run.py --repo . --output "$RUNNER_TEMP/typed-project-unit2-seal-results" --rustc rustc`

### quality-and-repository: Verify exact i32 literals against the independent Python integer oracle

- L136: `python3 scripts/verify_i32_literals.py target/release/oxid`

### quality-and-repository: Verify checked i32 arithmetic and debug-release parity with Python integers

- L139: `python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid`

### quality-and-repository: Verify every Oxid source, runnable program, README, SVG, build, and doctor check

- L142: `python3 scripts/verify_repo.py target/release/oxid`

### quality-and-repository: Test source installation

- L146: `cargo install --path . --locked --root "$RUNNER_TEMP/oxid-install"`

### quality-and-repository: Build container image

- L150: `docker build --tag oxid:ci .`

### quality-and-repository: Smoke-test container image

- L153: `docker run --rm oxid:ci --version`

### cross-platform: Build optimized runtime

- L188: `cargo build --release --locked`

### cross-platform: Run unit and native bridge tests

- L191: `cargo test --all-targets --locked`

### cross-platform: Verify exact typed formatter goldens

- L194: `python scripts/verify_typed_formatter.py ${{ matrix.binary }}`

### cross-platform: Test bounded parser selection and strict decoder controls

- L198: `python -B -m unittest discover -s scripts -p test_bounded_typed_parser.py -v`
- L199: `python -B -m unittest discover -s scripts -p test_parser_ast_observation.py -v`
- L200: `python -B -m unittest discover -s scripts -p test_static_resolution_observation.py -v`
- L201: `python -B -m unittest discover -s scripts -p test_static_typed_observation.py -v`
- L202: `python -B -m unittest discover -s scripts -p test_bounded_typed_static.py -v`

### cross-platform: Run Oxid language tests

- L205: `cargo run --release --locked -- test`

### cross-platform: Run concise syntax example

- L208: `cargo run --release --locked -- run examples/oxid_shortcuts.ox`

### cross-platform: Build the repository bundle

- L211: `cargo run --release --locked -- build`

### cross-platform: Check runtime version

- L214: `cargo run --release --locked -- --version`

### cross-platform: Verify compiler-demo artifact serialization round-trip

- L217: `cargo run --release --locked -- bootstrap`

### cross-platform: Recheck artifact round-trip without rewriting artifacts

- L220: `cargo run --release --locked -- bootstrap --check`

### cross-platform: Run the bounded performance framework

- L223: `cargo run --release --locked -- bench --iterations 3 --json target/oxid-benchmark.json`

### cross-platform: Validate the benchmark JSON schema

- L226: `python scripts/verify_benchmark_report.py target/oxid-benchmark.json 3`

### compiler-artifact-equivalence: Verify byte-identical serialized compiler-demo artifacts

- L251: `python3 scripts/verify_bootstrap_artifacts.py compiler-artifacts 4`

### native-scalar-preview: Qualify archived Unit3 source, native and mutation cases

- L312: `"$unit3_python" -B "$unit3_repo/tests/fixtures/typed_project_source_binding/run.py" prepare-archived --repo "$unit3_repo" --output "$unit3_out/source-binding"`
- L314: `"$unit3_python" -B "$unit3_pkg/portable/verify_package.py" --package "$unit3_pkg" --manifest "$unit3_pkg/package-manifest.json" --receipt "$unit3_out/package-verification.json"`
- L317: `"$unit3_python" -B "$unit3_pkg/portable/native-v1/stage_llvm_runtime.py" --output "$unit3_out/llvm-runtime"`
- L320: `"$unit3_python" -B "$unit3_pkg/portable/transport.py" materialize --archive "$unit3_pkg/source-transport/sources.tar.gz" --manifest "$unit3_pkg/source-transport/source-transport.json" --requests "$unit3_pkg/source-transport/requests.jsonl" --output "$unit3_out/inputs"`
- L325: `"$unit3_python" -B "$unit3_pkg/portable/comparison-v1/portable_compare.py" prepare --component-root "$unit3_pkg/components/oracles" --component-manifest "$unit3_pkg/portable/comparison-v1/component-manifest.json" --materialization "$unit3_out/inputs/materialization.json" --parser "$unit3_pkg/components/observer/parse_debug.py" --output "$unit3_out/comparison-view"`
- L329: `--parser "$unit3_pkg/components/observer/parse_debug.py" --output "$unit3_out/comparison-view"`
- L331: `"$unit3_python" -B "$unit3_pkg/portable/bridge.py" --repo "$unit3_out/source-binding/archived-selected" --selected-manifest "$unit3_pkg/manifests/selected-current.json" --core-manifest "$unit3_pkg/manifests/core-v1.json" --platform-patch "$unit3_pkg/manifests/platform-scope.patch" --original-test "$unit3_pkg/manifests/project_execution_tests.core-v1.rs" --output "$unit3_out/bridge"`
- L338: `"$unit3_python" -B "$unit3_pkg/portable/assemble.py" --repo "$unit3_out/bridge/derived-core-v1" --core-manifest "$unit3_pkg/manifests/core-v1.json" --observer-root "$unit3_pkg/components/observer" --observer-freeze "$unit3_pkg/components/observer/adapter-freeze-v1.json" --output "$unit3_out/observer"`
- L345: `"$unit3_python" -B "$unit3_pkg/portable/build.py" --prepared-manifest "$unit3_out/observer/prepared-source.json" --output "$unit3_out/source-build-$unit3_profile" --target-dir "$unit3_out/source-target" --profile "$unit3_profile" --jobs 2 --rustc "$unit3_rustc" --cargo "$unit3_cargo" --rustdoc "$unit3_rustdoc" --cargo-home "$unit3_cargo_home" --cc "$unit3_cc" --cxx "$unit3_cxx" --ar "$unit3_ar"`
- L352: `"$unit3_python" -B "$unit3_pkg/portable/make_plan.py" --kind source --materialization "$unit3_out/inputs/materialization.json" --prepared "$unit3_out/observer/prepared-source.json" --wrapper "$unit3_pkg/portable/observe.py" --debug-build "$unit3_out/source-build-debug/build.json" --release-build "$unit3_out/source-build-release/build.json" --receipt-root "$unit3_out/source-receipts" --output "$unit3_out/source-plan.json"`
- L354: `--prepared "$unit3_out/observer/prepared-source.json" --wrapper "$unit3_pkg/portable/observe.py" --debug-build "$unit3_out/source-build-debug/build.json" --release-build "$unit3_out/source-build-release/build.json" --receipt-root "$unit3_out/source-receipts" --output "$unit3_out/source-plan.json"`
- L359: `"$unit3_python" -B "$unit3_pkg/portable/collect.py" --plan "$unit3_out/source-plan.json" --plan-sha256 "$unit3_source_plan_sha" --python "$unit3_python" --output "$unit3_out/source-collection"`
- L362: `"$unit3_python" -B "$unit3_pkg/portable/native-v1/auxiliary_transport.py" --auxiliary-root "$unit3_pkg/components/native-driver/auxiliary" --original-manifest "$unit3_pkg/components/native-driver/auxiliary-manifest-v1.json" --output "$unit3_out/native-auxiliary"`
- L366: `"$unit3_python" -B "$unit3_pkg/portable/native-v1/native_inputs.py" --bridge-receipt "$unit3_out/bridge/bridge-receipt.json" --core-manifest "$unit3_pkg/manifests/core-v1.json" --overlay-manifest "$unit3_pkg/components/native-driver/overlay-manifest-v2.json" --overlay-patch "$unit3_pkg/components/native-driver/overlay-v2.patch" --auxiliary-root "$unit3_out/native-auxiliary/inputs" --output "$unit3_out/native-inputs"`
- L372: `"$unit3_python" -B "$unit3_pkg/portable/native-v1/native_portable_grouped.py" prepare --repo "$unit3_out/native-inputs/inputs" --core-manifest "$unit3_pkg/manifests/core-v1.json" --native-root "$unit3_pkg/components/native-driver" --native-manifest "$unit3_pkg/components/native-driver/artifact-manifest.json" --oracle-manifest "$unit3_pkg/components/oracles/pre-execution-manifest.json" --native-supplement "$unit3_pkg/components/oracles/supplements/native-diagnostics-v1/supplement-manifest.json" --native-fuel-requests "$unit3_pkg/components/oracles/supplements/native-diagnostics-v1/fuel-requests.jsonl" --materialization "$unit3_out/inputs/materialization.json" --output "$unit3_out/native"`
- L380: `"$unit3_python" -B "$unit3_pkg/portable/native-v1/native_portable_grouped.py" build --prepared-manifest "$unit3_out/native/native-prepared.json" --profile both --rustc "$unit3_rustc" --llvm-bin "$unit3_llvm_bin" --llvm-lib-dir "$unit3_llvm_lib_dir" --cargo-home "$unit3_cargo_home" --rustup-home "$unit3_rustup_home" --receipt "$unit3_out/native-build-wrapper.json"`
- L385: `"$unit3_python" -B "$unit3_pkg/portable/make_plan.py" --kind native --materialization "$unit3_out/inputs/materialization.json" --prepared "$unit3_out/native/native-prepared.json" --wrapper "$unit3_pkg/portable/native-v1/native_portable_grouped.py" --debug-build "$unit3_out/native/native-component/build-debug/verified-build.json" --release-build "$unit3_out/native/native-component/build-release/verified-build.json" --build-wrapper "$unit3_out/native-build-wrapper.json" --receipt-root "$unit3_out/native-wrappers" --output "$unit3_out/native-plan.json"`
- L387: `--wrapper "$unit3_pkg/portable/native-v1/native_portable_grouped.py" --debug-build "$unit3_out/native/native-component/build-debug/verified-build.json" --release-build "$unit3_out/native/native-component/build-release/verified-build.json" --build-wrapper "$unit3_out/native-build-wrapper.json" --receipt-root "$unit3_out/native-wrappers" --output "$unit3_out/native-plan.json"`
- L393: `"$unit3_python" -B "$unit3_pkg/portable/collect.py" --plan "$unit3_out/native-plan.json" --plan-sha256 "$unit3_native_plan_sha" --python "$unit3_python" --output "$unit3_out/native-collection" --rustc "$unit3_rustc" --llvm-bin "$unit3_llvm_bin" --llvm-lib-dir "$unit3_llvm_lib_dir" --cargo-home "$unit3_cargo_home" --rustup-home "$unit3_rustup_home"`
- L398: `"$unit3_python" -B "$unit3_pkg/portable/mutations.py" prepare --variant "$unit3_variant" --observer-prepared "$unit3_out/observer/prepared-source.json" --component-root "$unit3_pkg/components/mutations/$unit3_variant" --oracle-manifest "$unit3_pkg/components/oracles/pre-execution-manifest.json" --mutation-requests "$unit3_pkg/components/oracles/mutation-requests.json" --effective-requests "$unit3_pkg/components/oracles/supplements/mutation-applicability-v1/requests.json" --applicability-manifest "$unit3_pkg/components/oracles/supplements/mutation-applicability-v1/supplement-manifest.json" --output "$unit3_out/mutations-$unit3_variant"`
- L407: `"$unit3_python" -B "$unit3_pkg/portable/build.py" --prepared-manifest "$unit3_out/mutations-$unit3_variant/prepared-mutation.json" --output "$unit3_out/mutation-build-$unit3_variant-$unit3_profile" --target-dir "$unit3_out/mutations-$unit3_variant/target" --profile "$unit3_profile" --jobs 2 --rustc "$unit3_rustc" --cargo "$unit3_cargo" --rustdoc "$unit3_rustdoc" --cargo-home "$unit3_cargo_home" --cc "$unit3_cc" --cxx "$unit3_cxx" --ar "$unit3_ar"`
- L415: `"$unit3_python" -B "$unit3_pkg/portable/mutation_gate.py" make --materialization "$unit3_out/inputs/materialization.json" --wrapper "$unit3_pkg/portable/mutations.py" --v2-prepared "$unit3_out/mutations-v2/prepared-mutation.json" --v3-prepared "$unit3_out/mutations-v3/prepared-mutation.json" --v2-debug-build "$unit3_out/mutation-build-v2-debug/build.json" --v2-release-build "$unit3_out/mutation-build-v2-release/build.json" --v3-debug-build "$unit3_out/mutation-build-v3-debug/build.json" --v3-release-build "$unit3_out/mutation-build-v3-release/build.json" --receipt-root "$unit3_out/mutation-receipts" --output "$unit3_out/mutation-plan.json"`
- L416: `--materialization "$unit3_out/inputs/materialization.json" --wrapper "$unit3_pkg/portable/mutations.py" --v2-prepared "$unit3_out/mutations-v2/prepared-mutation.json" --v3-prepared "$unit3_out/mutations-v3/prepared-mutation.json" --v2-debug-build "$unit3_out/mutation-build-v2-debug/build.json" --v2-release-build "$unit3_out/mutation-build-v2-release/build.json" --v3-debug-build "$unit3_out/mutation-build-v3-debug/build.json" --v3-release-build "$unit3_out/mutation-build-v3-release/build.json" --receipt-root "$unit3_out/mutation-receipts" --output "$unit3_out/mutation-plan.json"`
- L425: `"$unit3_python" -B "$unit3_pkg/portable/mutation_gate.py" collect --plan "$unit3_out/mutation-plan.json" --plan-sha256 "$unit3_mutation_plan_sha" --python "$unit3_python" --output "$unit3_out/mutation-collection"`
- L428: `"$unit3_python" -B "$unit3_pkg/portable/comparison-v1/portable_compare.py" compare --prepared "$unit3_out/comparison-view/prepared-comparison.json" --plan "$unit3_out/source-plan.json" --plan-sha256 "$unit3_source_plan_sha" --output "$unit3_out/source-comparison"`
- L432: `"$unit3_python" -B "$unit3_pkg/portable/comparison-v1/portable_compare.py" compare --prepared "$unit3_out/comparison-view/prepared-comparison.json" --plan "$unit3_out/native-plan.json" --plan-sha256 "$unit3_native_plan_sha" --output "$unit3_out/native-comparison"`
- L437: `"$unit3_python" -B "$unit3_pkg/portable/comparison-v1/portable_compare.py" compare --prepared "$unit3_out/comparison-view/prepared-comparison.json" --plan "$unit3_out/mutation-plan.json" --plan-sha256 "$unit3_mutation_plan_sha" --native-evidence "$unit3_out/native-comparison/comparison.json" --native-evidence-sha256 "$unit3_native_evidence_sha" --output "$unit3_out/mutation-comparison"`

### native-scalar-preview: Surface bounded native build failure details

- L448: `python3 -B scripts/collect_unit3_ci_diagnostics.py failure-logs`

### native-scalar-preview: Prepare bounded Unit3 diagnostic receipts

- L455: `python3 -B scripts/collect_unit3_ci_diagnostics.py compact`

### native-scalar-preview: Index and preserve Unit3 results on every terminal state

- L474: `python3 -B scripts/preserve_unit3_ci_evidence.py`

### native-scalar-preview: Build the compiler and run admission/tool-boundary regressions

- L489: `cargo test --locked --test typed_native`
- L490: `cargo build --release --locked`
- L491: `cargo test --locked --bin oxid frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm -- --ignored --exact`
- L492: `cargo test --release --locked --bin oxid frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm -- --ignored --exact`
- L493: `cargo test --locked --bin oxid frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm -- --ignored --exact`
- L494: `cargo test --release --locked --bin oxid frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm -- --ignored --exact`
- L495: `cargo test --locked --bin oxid frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm -- --ignored --exact`
- L496: `cargo test --release --locked --bin oxid frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm -- --ignored --exact`
- L497: `cargo test --locked --bin oxid frontend::oir::loop_control_tests::loop_control_uses_real_llvm -- --ignored --exact`
- L498: `cargo test --release --locked --bin oxid frontend::oir::loop_control_tests::loop_control_uses_real_llvm -- --ignored --exact`
- L499: `cargo test --locked --bin oxid frontend::oir::owned::source::array_consumer_tests::source_array_consumers_use_real_llvm -- --ignored --exact`
- L500: `cargo test --release --locked --bin oxid frontend::oir::owned::source::array_consumer_tests::source_array_consumers_use_real_llvm -- --ignored --exact`

### native-scalar-preview: Qualify experimental checked-HIR import native paths in both profiles

- L512: `cargo test "${profile_args[@]}" --locked --test typed_hir_import imported_public_compile_produces_source_independent_executable -- --ignored --exact --nocapture > "$proof_root/$profile/public.stdout" 2> "$proof_root/$profile/public.stderr"`
- L517: `cargo test "${profile_args[@]}" --locked --bin oxid --no-run --message-format=json > "$proof_root/$profile/build.jsonl" 2> "$proof_root/$profile/build.stderr"`
- L536: `python3 -B scripts/verify_checked_hir_import_native.py --test-binary "$binary" --llvm-bin "$OXID_LLVM_BIN" --evidence "$proof_root/$profile/private"`

### native-scalar-preview: Verify public fixed-array CLI native parity in both profiles

- L550: `cargo test --locked --test typed_fixed_arrays public_array_cli_compiles_source_free_native_parity -- --ignored --exact`
- L551: `cargo test --release --locked --test typed_fixed_arrays public_array_cli_compiles_source_free_native_parity -- --ignored --exact`
- L552: `cargo test --locked --test typed_fixed_arrays public_array_cli_native_module_and_fuel_parity -- --ignored --exact`
- L553: `cargo test --release --locked --test typed_fixed_arrays public_array_cli_native_module_and_fuel_parity -- --ignored --exact`

### native-scalar-preview: Test producer-native admission and evidence rejection controls

- L555: `python3 -B -m unittest discover -s scripts -p test_owned_array_native.py -v`

### native-scalar-preview: Admit and retain the existing owned-native prefix in both profiles

- L563: `python3 -B scripts/verify_owned_array_native.py run --repo "$PWD" --output "$RUNNER_TEMP/fixed-array-unit2e/producer" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --cargo "$(rustup which cargo)" --rustc "$(rustup which rustc)" --cargo-home "${CARGO_HOME:-$HOME/.cargo}" --llvm-bin "$OXID_LLVM_BIN" --profile both`

### native-scalar-preview: Run and retain complete independent Unit2D replay (debug)

- L577: `python3 -B scripts/verify_owned_array_native.py independent --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" --output "$RUNNER_TEMP/fixed-array-unit2e/independent/debug" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --profile debug`

### native-scalar-preview: Run and retain complete independent Unit2D replay (release)

- L589: `python3 -B scripts/verify_owned_array_native.py independent --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" --output "$RUNNER_TEMP/fixed-array-unit2e/independent/release" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --profile release`

### native-scalar-preview: Verify held-out owned-source native execution

- L596: `OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE="$PWD/target/source-review-debug" cargo test --locked --bin oxid frontend::oir::owned::source::reviewer_source::reviewer_source_native_heldout_successes_and_failures -- --ignored --exact --nocapture`
- L597: `OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE="$PWD/target/source-review-release" cargo test --release --locked --bin oxid frontend::oir::owned::source::reviewer_source::reviewer_source_native_heldout_successes_and_failures -- --ignored --exact --nocapture`

### native-scalar-preview: Verify checked division and remainder native parity in both profiles

- L600: `cargo test --locked --test typed_division -- --ignored`
- L601: `cargo test --release --locked --test typed_division -- --ignored`

### native-scalar-preview: Verify checked unary negation source-free parity in both profiles

- L604: `OXID_UNARY_NATIVE_EVIDENCE="$PWD/target/checked-unary-native/debug-raw" cargo test --locked --bin oxid raw_negation -- --include-ignored --test-threads=1`
- L605: `OXID_UNARY_NATIVE_EVIDENCE="$PWD/target/checked-unary-native/release-raw" cargo test --release --locked --bin oxid raw_negation -- --include-ignored --test-threads=1`
- L606: `OXID_UNARY_NATIVE_EVIDENCE="$PWD/target/checked-unary-native/debug-source" cargo test --locked --test typed_unary_negation -- --ignored --test-threads=1`
- L607: `OXID_UNARY_NATIVE_EVIDENCE="$PWD/target/checked-unary-native/release-source" cargo test --release --locked --test typed_unary_negation -- --ignored --test-threads=1`

### native-scalar-preview: Verify borrowed scalar slices native parity in both profiles

- L618: `cargo test --locked --test typed_slices -- --ignored`
- L619: `cargo test --release --locked --test typed_slices -- --ignored`
- L620: `cargo test --locked --bin oxid native_slices -- --ignored`
- L621: `cargo test --release --locked --bin oxid native_slices -- --ignored`

### native-scalar-preview: Verify owned record composition native parity in both profiles

- L624: `cargo test --locked --test typed_record_composition -- --ignored --test-threads=1`
- L625: `cargo test --release --locked --test typed_record_composition -- --ignored --test-threads=1`
- L626: `cargo test --locked --bin oxid native_composition -- --include-ignored --test-threads=1`
- L627: `cargo test --release --locked --bin oxid native_composition -- --include-ignored --test-threads=1`

### native-scalar-preview: Verify native inventory source-free admission in both profiles

- L630: `OXID_OWNED_NATIVE_EVIDENCE="$PWD/target/native-inventory/debug" cargo test --locked --bin oxid native_inventory -- --include-ignored --test-threads=1 --nocapture`
- L631: `OXID_OWNED_NATIVE_EVIDENCE="$PWD/target/native-inventory/release" cargo test --release --locked --bin oxid native_inventory -- --include-ignored --test-threads=1 --nocapture`

### native-scalar-preview: Verify projected array slice source-free parity in both profiles

- L642: `OXID_OWNED_NATIVE_EVIDENCE="$PWD/target/projected-slices/debug-raw" cargo test --locked --bin oxid native_projected_slices -- --include-ignored --test-threads=1`
- L643: `OXID_OWNED_NATIVE_EVIDENCE="$PWD/target/projected-slices/release-raw" cargo test --release --locked --bin oxid native_projected_slices -- --include-ignored --test-threads=1`
- L644: `OXID_PROJECTED_NATIVE_EVIDENCE="$PWD/target/projected-slices/debug-source" cargo test --locked --test typed_projected_slices -- --ignored --test-threads=1`
- L645: `OXID_PROJECTED_NATIVE_EVIDENCE="$PWD/target/projected-slices/release-source" cargo test --release --locked --test typed_projected_slices -- --ignored --test-threads=1`

### native-scalar-preview: Verify bounded Oxid expression parser and evaluator

- L656: `python3 -B scripts/verify_expression_component.py --oxid target/release/oxid --output "$RUNNER_TEMP/expression-component" --native`

### native-scalar-preview: Test bounded enum native admission controls

- L668: `python3 -B -m unittest discover -s scripts -p test_bounded_enum_native.py -v`

### native-scalar-preview: Verify exact bounded enum native groups and public CLI in both profiles

- L673: `python3 -B scripts/verify_bounded_enum_native.py --repo "$PWD" --output "$RUNNER_TEMP/bounded-enum-native" --cargo "$(rustup which cargo)" --llvm-bin "$OXID_LLVM_BIN" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA"`

### native-scalar-preview: Test bounded u8 native, resource, and exhaustive-oracle admission

- L687: `python3 -B -m unittest discover -s scripts -p test_bounded_u8_native.py -v`
- L688: `python3 -O -B -m unittest discover -s scripts -p test_bounded_u8_native.py -v`
- L689: `python3 -B -m unittest discover -s scripts -p test_qualify_hir_u8_compatibility.py -v`

### native-scalar-preview: Verify bounded u8 exact tests, both exhaustive routes, and frozen providers in both profiles

- L694: `python3 -B scripts/verify_bounded_u8_native.py --repo "$PWD" --output "$RUNNER_TEMP/bounded-u8-native" --build-evidence "$RUNNER_TEMP/bounded-enum-native" --llvm-bin "$OXID_LLVM_BIN" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA"`

### native-scalar-preview: Test bounded byte storage closed oracle and exact-source admission

- L710: `python3 -B -m unittest discover -s scripts -p test_bounded_byte_storage.py -v`
- L711: `python3 -O -B -m unittest discover -s scripts -p test_bounded_byte_storage.py -v`
- L712: `python3 -B -m unittest discover -s scripts -p test_qualify_hir_byte_storage_compatibility.py -v`
- L713: `python3 -O -B -m unittest discover -s scripts -p test_qualify_hir_byte_storage_compatibility.py -v`

### native-scalar-preview: Verify bounded byte storage resources, reference and native oracles in both profiles

- L725: `python3 -B scripts/verify_bounded_byte_storage.py --prepare-public-build --repo "$PWD" --build-evidence "$RUNNER_TEMP/bounded-enum-native" --cargo "$byte_storage_cargo" --expected-head "$UNIT2E_EXPECTED_HEAD" --output "$RUNNER_TEMP/byte-storage-public-builds"`
- L729: `python3 -B scripts/verify_bounded_byte_storage.py --repo "$PWD" --build-evidence "$RUNNER_TEMP/bounded-enum-native" --public-build-evidence "$RUNNER_TEMP/byte-storage-public-builds" --llvm-bin "$OXID_LLVM_BIN" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --output "$RUNNER_TEMP/byte-storage-current"`

### native-scalar-preview: Test bounded stdin controller admission

- L746: `python3 -B tests/qualification/bounded_stdin_current/test_controller.py -v`

### native-scalar-preview: Verify bounded stdin effects and fuel in both ordinary profiles

- L751: `python3 -B scripts/verify_bounded_stdin_native.py --repo "$PWD" --output "$RUNNER_TEMP/bounded-stdin-native" --build-evidence "$RUNNER_TEMP/bounded-enum-native" --llvm-bin "$OXID_LLVM_BIN" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --source-manifest-sha256 402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa`

### native-scalar-preview: Test bounded stdout controller and artifact admission

- L758: `python3 -B tests/qualification/bounded_stdout_current/test_controller.py -v`
- L759: `python3 -B -m unittest discover -s scripts -p test_bounded_stack_artifact.py -v`

### native-scalar-preview: Verify bounded stdout and public artifacts in both ordinary profiles

- L764: `python3 -B scripts/verify_bounded_stdout_native.py --repo "$PWD" --output "$RUNNER_TEMP/bounded-stdout-native" --build-evidence "$RUNNER_TEMP/bounded-enum-native" --llvm-bin "$OXID_LLVM_BIN" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --source-manifest-sha256 402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa`

### native-scalar-preview: Verify one expression ELF across bounded stdin inputs

- L779: `python3 -B scripts/verify_expression_stdin.py --oxid target/release/oxid --output "$RUNNER_TEMP/expression-stdin" --native`

### native-scalar-preview: Verify bounded stack-code rows and independent malformed programs

- L794: `python3 -B scripts/verify_stack_component.py --oxid target/release/oxid --output "$RUNNER_TEMP/stack-component" --native`
- L797: `python3 -B scripts/verify_expression_stdin.py --oxid target/release/oxid --output "$RUNNER_TEMP/stack-stdin" --native --component stack`

### native-scalar-preview: Verify bounded typed lexer against the canonical lexer

- L817: `python3 -B scripts/build_typed_lexer_observer.py --output "$RUNNER_TEMP/typed-lexer-observer"`
- L819: `python3 -B scripts/verify_bounded_typed_lexer.py --oxid target/debug/oxid --observer "$RUNNER_TEMP/typed-lexer-observer/canonical-lexer-observer" --output "$RUNNER_TEMP/typed-lexer-proof" --native`

### native-scalar-preview: Verify bounded scalar parser against complete canonical observations

- L837: `python3 -B -m unittest discover -s scripts -p test_bounded_typed_parser.py -v`
- L838: `python3 -B -m unittest discover -s scripts -p test_parser_ast_observation.py -v`
- L839: `python3 -B scripts/build_typed_parser_observer.py --output "$RUNNER_TEMP/typed-parser-observer"`
- L841: `python3 -B tests/fixtures/bounded_typed_parser/observer/test_observer.py --observer "$RUNNER_TEMP/typed-parser-observer/canonical-parser-observer" --report "$RUNNER_TEMP/typed-parser-observer/projection-tests.json"`
- L845: `python3 -B scripts/verify_bounded_typed_parser.py --oxid "target/$profile/oxid" --parser-observer "$RUNNER_TEMP/typed-parser-observer/canonical-parser-observer" --lexer-observer "$RUNNER_TEMP/typed-lexer-observer/canonical-lexer-observer" --output "$RUNNER_TEMP/typed-parser-proof-$profile" --native`

### native-scalar-preview: Verify bounded scalar static frontend and untrusted AST boundary

- L867: `python3 -B -m unittest discover -s scripts -p test_static_resolution_observation.py -v`
- L868: `python3 -B -m unittest discover -s scripts -p test_static_typed_observation.py -v`
- L869: `python3 -B -m unittest discover -s scripts -p test_bounded_typed_static.py -v`
- L870: `python3 -B scripts/build_typed_static_observer.py --output "$RUNNER_TEMP/typed-static-observer"`
- L873: `python3 -B tests/fixtures/bounded_typed_static/observer/test_observer.py --observer "$RUNNER_TEMP/typed-static-observer/canonical-static-observer" --canonical "target/$profile/oxid" --evidence "$RUNNER_TEMP/typed-static-observer-tests-$profile"`
- L877: `python3 -B scripts/verify_bounded_typed_static.py --oxid "target/$profile/oxid" --static-observer "$RUNNER_TEMP/typed-static-observer/canonical-static-observer" --lexer-observer "$RUNNER_TEMP/typed-lexer-observer/canonical-lexer-observer" --output "$RUNNER_TEMP/typed-static-proof-$profile" --native`

### native-scalar-preview: Verify production source CLI and source-free ELF outcomes

- L897: `python3 scripts/verify_owned_source.py target/debug/oxid target/release/oxid --mode cli --cli-amendment projected-array-slices-v1 --jobs 2`

### native-scalar-preview: Verify owned-source native budgets, stores, guards, and artifact identities

- L899: `python3 scripts/verify_owned_source_native.py --llvm-bin "$OXID_LLVM_BIN" --jobs 2 --evidence target/owned-source-native-ci`

### native-scalar-preview: Verify actual LLVM compilation, reference parity, and ELF artifacts

- L906: `python3 scripts/verify_native_suite.py target/debug/oxid target/release/oxid --jobs 2`

### native-scalar-preview: Index and preserve combined Unit2E evidence on every terminal state

- L918: `python3 -B scripts/verify_owned_array_native.py package --producer-root "$RUNNER_TEMP/fixed-array-unit2e/producer" --output "$RUNNER_TEMP/fixed-array-unit2e-upload" --expected-head "$UNIT2E_EXPECTED_HEAD" --event-sha "$UNIT2E_EVENT_SHA" --producer-step-outcome "$UNIT2E_PRODUCER_OUTCOME" --independent-root "$RUNNER_TEMP/fixed-array-unit2e/independent" --independent-debug-outcome "$UNIT2E_INDEPENDENT_DEBUG_OUTCOME" --independent-release-outcome "$UNIT2E_INDEPENDENT_RELEASE_OUTCOME"`

### unit4-linux: Verify Unit4 integration failure controls

- L988: `python3 -B tests/qualification/unit4_ci/test_integration.py`

### unit4-linux: Execute complete current-source public, hosted-root and parser gates

- L997: `python3 -B tests/qualification/unit4_ci/gate.py host --repo "$GITHUB_WORKSPACE/unit4-current" --output "$RUNNER_TEMP/unit4-linux" --expected-head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --host 'Linux x86_64' --historical-repo "$RUNNER_TEMP/unit4-parser-authority" --toolchain "$unit4_toolchain" --cargo-cache "${CARGO_HOME:-$HOME/.cargo}"`

### unit4-linux: Preserve complete or failed Unit4 evidence

- L1005: `python3 -B tests/qualification/unit4_ci/evidence.py --output "$RUNNER_TEMP/unit4-linux" --archives "$RUNNER_TEMP/unit4-linux-archives"`

### unit4-linux: Stage the closed compact Linux upload directory

- L1008: `python3 -B tests/qualification/unit4_ci/evidence.py --archives "$RUNNER_TEMP/unit4-linux-archives" --stage-compact-upload "$RUNNER_TEMP/unit4-linux-compact-upload"`

### unit4-portable-hosts: Restore and verify tracked committed bytes through a fresh checkout

- L1058: `python -B tests/qualification/unit4_ci/common.py --restore-checkout --repo "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/unit4-committed-checkout" --expected-head "$UNIT4_EXPECTED_HEAD"`

### unit4-portable-hosts: Verify actual-host integration controls

- L1071: `python -B tests/qualification/unit4_ci/test_integration.py`

### unit4-portable-hosts: Execute ordinary, lifecycle and actual no-native trap gates

- L1079: `python -B tests/qualification/unit4_ci/gate.py host --repo "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/unit4-host" --expected-head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --host "$UNIT4_HOST"`

### unit4-portable-hosts: Execute Windows gates with the native toolchain search path

- L1097: `python -B tests/qualification/unit4_ci/gate.py host --repo "$env:GITHUB_WORKSPACE" --output "$env:RUNNER_TEMP/unit4-host" --expected-head "$env:UNIT4_EXPECTED_HEAD" --event-sha "$env:UNIT4_EVENT_SHA" --host "$env:UNIT4_HOST"`

### unit4-portable-hosts: Preserve complete or failed actual-host evidence

- L1106: `python -B tests/qualification/unit4_ci/evidence.py --output "$RUNNER_TEMP/unit4-host" --archives "$RUNNER_TEMP/unit4-host-archives"`

### unit4-qualification: Require complete raw frozen rosters from all four actual hosts

- L1155: `python -B tests/qualification/unit4_ci/join.py --repo "$GITHUB_WORKSPACE" --artifacts "$RUNNER_TEMP/unit4-host-artifacts" --output "$RUNNER_TEMP/unit4-final-join" --expected-head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --job-results "$UNIT4_JOB_RESULTS"`

### local-hir-producers: Test producer qualification admission and evidence controls

- L1200: `python3 -B -m unittest discover -s scripts -p test_qualify_hir_producers.py -v`
- L1201: `python3 -B -m unittest discover -s scripts -p test_run_hir_producer_ci.py -v`
- L1202: `python3 -B -m unittest discover -s scripts -p test_qualify_hir_producers_v2.py -v`
- L1203: `python3 -O -B -m unittest discover -s scripts -p test_qualify_hir_producers_v2.py -v`
- L1204: `python3 -O -B -m unittest discover -s scripts -p test_run_hir_producer_ci.py -v`

### local-hir-producers: Build exact-head executables and qualify v1, v2 and edge controls in both ordinary profiles

- L1207: `python3 -B scripts/run_hir_producer_ci.py --expected-head "$HIR_PRODUCER_EXPECTED_HEAD" --event-sha "$HIR_PRODUCER_EVENT_SHA" --llvm-bin "$OXID_LLVM_BIN" --output "$RUNNER_TEMP/local-hir-producers"`

### streaming-lexical-provider: Test lexical framing and qualification admission

- L1255: `python3 -B -m unittest discover -s scripts -p test_streaming_lexer_protocol.py -v`
- L1256: `python3 -B -m unittest discover -s scripts -p test_qualify_streaming_lexer.py -v`
- L1257: `python3 -O -B -m unittest discover -s scripts -p test_qualify_streaming_lexer.py -v`

### streaming-lexical-provider: Rebuild actual producer closures through selected lexical tokens and test failures

- L1260: `python3 -B scripts/run_streaming_lexer_ci.py --expected-head "$LEXICAL_EXPECTED_HEAD" --event-sha "$LEXICAL_EVENT_SHA" --llvm-bin "$OXID_LLVM_BIN" --output "$RUNNER_TEMP/streaming-lexical-provider"`

Workflow SHA-256 at audit: `7ae94a3f70dd543a28b780912d7ca48fced54d39e0ba0cfdcd2152b5d5b10c57`.
