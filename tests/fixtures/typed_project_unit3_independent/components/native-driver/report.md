# Unit3 private native and driver qualification

The reviewed finite native/driver roster completed in both Rust 1.99.0 compiler profiles. The independent comparator matched every recorded stdout, stderr, and exit status to the source-only frozen expectations. Production code, existing raw verifiers, public syntax admission, and frozen oracle files were not edited.

Final independent core-review sign-off is recorded separately when received. This report does not qualify the entire 72-fixture family for native execution, publish public project syntax, establish full raw/event correspondence, or claim repository-wide CI/merge readiness.

## Exact counts

Per profile, the 150 primary invocations are:

| Category | Invocations per profile | Actual source-free ELF executions per profile |
| --- | ---: | ---: |
| Default native executable descriptors | 22 | 22 |
| Default native admission-only descriptors | 10 | 0 |
| Selected reduced-fuel native descriptors | 18 | 18 |
| Paired reduced-fuel text reference runs | 18 | 0 |
| Private driver: 12 fixtures × 3 operations × 2 formats | 72 | 8 supplemental |
| No-clobber: 8 admitted file/symlink/format controls + 2 recursion-priority controls | 10 | 0 |
| Total per profile | 150 | 48 |

Across both profiles there are 300 primary invocations and 96 actual ELF executions: 44 default, 36 selected-fuel, and 16 supplemental private-driver executions. All 16 supplemental outputs were actually run alone in fresh directories with source roots renamed; they are not merely compiled artifacts. Six retained initial debug smoke invocations add three separately counted ELF executions. The independent semantic comparator covers all 306 receipts and all 99 actual executions.

The 32 default descriptors are exactly 22 executable plus 10 admission-only. The executable subset contains seven of the 72 source/model family fixtures, five project pilots/control, eight scalar/owned overflow layout cases, and two equal-offset different-file collision cases. Admission-only cases contain two imported-main controls, four used/unused scalar/owned recursion controls, and four missing-root layout controls. Exact roster: `native-requests.json`.

## What was exercised

The adapter enters actual `options::route`, loads real separate files with the private project loader, checks once with the existing project executable facade, then passes that result once to unchanged `driver::process_loaded`. Public dispatch remains original-only. Invalid argument handling delegates only an already verified `Route::Error` branch to original dispatch. Parent-owned current-core public compatibility evidence is separate; this adapter's dormant public mode is not counted as public CLI evidence.

After loading, every invocation renames its private fixture root before source checking, reference execution, or native IR construction. Every loaded display path is verified absent. Each fixture tree is restored with exact original file membership and source bytes. All successful native paths use the actual existing `frontend::native::compile`; the passive capture records the module passed to it. Real LLVM 19.1.7 verifies the IR, compiles IR/runtime at O0, and links Linux x86_64 ELF. Each successful compile records seven actual external command invocations. Each ELF is copied alone to a fresh directory, its source root is renamed again, and it is run with only PATH=/usr/bin:/bin, LANG=C, LC_ALL=C. Both source-unavailable transitions and restoration are retained.

The independent diagnostic supplement freezes exact human/JSON streams before candidate execution, with only pre-agreed literal fixture-root and output-path substitution. It covers scalar/owned arithmetic overflow under ASCII/Unicode and LF/CRLF layouts, equal-offset origins in different files, native whole-program recursion and entry admission, source type/privacy/ownership denials, reference runtime errors, driver summaries, and exit policy. Both collision controls compile successfully and then fail at runtime with their prescribed E0604 diagnostics; they are not compilation rejections.

The corrected 18-case fuel subset was frozen from unchanged independent source-model schedules before execution. Scalar uses 164/165. Mechanical Batch uses 201/281/204/1255/1296/1297. Opaque Batch uses 280/377/244/1528/1576/1577. Loop cleanup uses 149/337/353/354. This covers selected write, nested Invoke, normal return, lexical cleanup, continue/break cleanup, exact-budget success, and one-below failure diagnostics. All native and paired-reference streams match. Native output parity alone does not establish complete committed-state/event correspondence; that remains the separate observer/comparator gate.

All check/reference calls and all declared rejection controls have initialized empty external-tool logs. Check/reference directories contain only explicit evidence receipts, with no program/IR/output/cache; restored fixture trees contain exactly their declared files. Admitted scalar/Counter regular-file and symlink outputs remain byte-identical in both formats. Scalar/owned recursion controls reject before occupied-output handling and before any LLVM invocation. Rejecting spy tools are used only to detect forbidden calls and never provide successful native evidence.

## Identity and implementation limits

The original checkout HEAD is 9e61a384e49302db6f47cde4159ae94538bb020f. The 117-file working core is bound by source-inputs-core-v1 SHA-256 53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910. Post-execution verification confirms all 117 core inputs and all 445 original frozen source files unchanged.

The original oracle manifest remains b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba. Native diagnostic/fuel supplement is 9cae46e804a2776eb44732ced7201f99e37312346ef81958d025ee255f644c0a. The compiler adapter never reads expected values. Its invocation controller reads only exact, hash-checked source-only projections.

The cfg(test)-only overlay adds four limit clamps at existing scalar/owned reference/native entrances, a private driver child adapter, and passive actual IR/external-command recording. It does not expose a checked witness, substitute map/body/entry, replace a resolver, or change default admission. Overlay patch: 2df49ce00cbb0e4483a9018b90279cf6b9d42c204c767692d0e740493e507bee. Final controller: 05c96a02ba8ff2d5069a45e737aeefd37dfabf8d65fd16c9cffd8b5147089306.

Successful profile build receipts bind source manifest, overlay manifest, controller/main/request manifest, rustc identity, runtime identity, flags, and binary hash. Every invocation revalidates those bindings. Debug binary: 0fdfdcd941faf59c45c6c6dc8fd02c6fe4a914bdc24ff38c44943fbe09a156eb. Release binary: 2277e46b305038a210e93cd6c10f748b0eb5feabad467097e63f8b63aeb022cf. Builds took 3.827 and 9.133 seconds and had empty stderr. No Cargo build or incremental cache was used; codegen units were limited to two. LLVM/rustc binary hashes are rechecked after execution.

## Independent results and retained failures

`independent-semantic-comparison.json` is a byte-identical copy of the independently authored comparison receipt, SHA-256 45c95bb083485e822484a272f56bd47e5425f21f036654f56b0d59bdbeb44609. It binds all 306 receipt hashes and the frozen supplement. Comparator SHA-256 is 4a85c10ed25ac11158f17d4c6a01bf06f42a8ad87f1e2ce4fd8552ba6343ab5e. Every driver stream is EXACT_MATCH; every executed ELF stream is EXACT_MATCH. The source-independent transport audit reports zero missing, unexpected, duplicate, or protocol-failed receipts.

Two source-only setup failures remain in `setup-attempts.jsonl`: initial fixture-copy directory collision (partial source-v1 and initial preparer retained), and initial attribution JSON key assumption (corrected from the inspected source-only shape). Both occurred with zero candidate/compiler executions. No candidate build failed, no candidate timed out or needed source rescue, and no observed candidate mismatch was erased or widened. The independent comparator's two implementation corrections (null output placeholder handling and distinguishing a compiled collision runtime error from compilation rejection) changed no expected outcome; its history belongs to the independently owned oracle comparison package.

`driver-mutation-attribution.json` binds the four original driver-boundary mutation request IDs to these existing actual receipts in both profiles. It adds zero executions and zero internal raw-IR mutation claims. The full 114-mutation gate remains independently aggregated with the other 110 controls.

Primary evidence: `protocol-audit.json`, `independent-semantic-comparison.json`, `post-execution-input-verification.json`, `driver-mutation-attribution.json`, profile `verified-build.json` files, `toolchain.json`, source/overlay/controller/input manifests, and the complete `receipts/` tree.
