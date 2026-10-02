# Unit3 portable qualification commands

The Linux x86_64 gate uses explicit official Rust 1.99.0 and LLVM 19.1.7 tools.
The ordinary jobs compile the actual selected successor. Semantic gates below
execute derived core-v1 with a production-byte identity bridge to that successor.
Keep the existing gates and all historical manifests unchanged.

Set these absolute paths in the workflow: `REPO`, `OUT` (fresh external output),
`PYTHON`, `RUSTC`, `CARGO`, `RUSTDOC`, `CARGO_HOME`, `RUSTUP_HOME`, `CC`, `CXX`,
`AR`, `LLVM_BIN`, `LLVM_LIB_DIR`. `PKG` is
`$REPO/tests/fixtures/typed_project_unit3_independent`.
Run every Python command with `PYTHONOPTIMIZE=0` and `-B`. Use at most two build
jobs and incremental compilation disabled. Fetch the repository's locked Cargo
dependencies before the offline builds. No source corpus is materialized inside
the repository. Keep source-input qualification before cache-producing steps.

```sh
"$PYTHON" -B "$PKG/portable/verify_package.py" --package "$PKG" \
  --manifest "$PKG/package-manifest.json" --receipt "$OUT/package-verification.json"

"$PYTHON" -B "$PKG/portable/transport.py" materialize \
  --archive "$PKG/source-transport/sources.tar.gz" \
  --manifest "$PKG/source-transport/source-transport.json" \
  --requests "$PKG/source-transport/requests.jsonl" --output "$OUT/inputs"

"$PYTHON" -B "$PKG/portable/bridge.py" --repo "$REPO" \
  --selected-manifest "$PKG/manifests/selected-current.json" \
  --core-manifest "$PKG/manifests/core-v1.json" \
  --platform-patch "$PKG/manifests/platform-scope.patch" \
  --original-test "$PKG/manifests/project_execution_tests.core-v1.rs" \
  --output "$OUT/bridge"

"$PYTHON" -B "$PKG/portable/assemble.py" \
  --repo "$OUT/bridge/derived-core-v1" --core-manifest "$PKG/manifests/core-v1.json" \
  --observer-root "$PKG/components/observer" \
  --observer-freeze "$PKG/components/observer/adapter-freeze-v1.json" \
  --output "$OUT/observer"

for PROFILE in debug release; do
  "$PYTHON" -B "$PKG/portable/build.py" \
    --prepared-manifest "$OUT/observer/prepared-source.json" \
    --output "$OUT/source-build-$PROFILE" --target-dir "$OUT/source-target" \
    --profile "$PROFILE" --jobs 2 --rustc "$RUSTC" --cargo "$CARGO" \
    --rustdoc "$RUSTDOC" --cargo-home "$CARGO_HOME" --cc "$CC" --cxx "$CXX" --ar "$AR"
done

"$PYTHON" -B "$PKG/portable/make_plan.py" --kind source \
  --materialization "$OUT/inputs/materialization.json" \
  --prepared "$OUT/observer/prepared-source.json" --wrapper "$PKG/portable/observe.py" \
  --debug-build "$OUT/source-build-debug/build.json" \
  --release-build "$OUT/source-build-release/build.json" \
  --receipt-root "$OUT/source-receipts" --output "$OUT/source-plan.json"
SOURCE_PLAN_SHA=$(sha256sum "$OUT/source-plan.json" | cut -d' ' -f1)
"$PYTHON" -B "$PKG/portable/collect.py" --plan "$OUT/source-plan.json" \
  --plan-sha256 "$SOURCE_PLAN_SHA" --python "$PYTHON" --output "$OUT/source-collection"

"$PYTHON" -B "$PKG/portable/native-v1/native_inputs.py" \
  --bridge-receipt "$OUT/bridge/bridge-receipt.json" \
  --core-manifest "$PKG/manifests/core-v1.json" \
  --overlay-manifest "$PKG/components/native-driver/overlay-manifest-v2.json" \
  --overlay-patch "$PKG/components/native-driver/overlay-v2.patch" \
  --auxiliary-root "$PKG/components/native-driver/auxiliary" --output "$OUT/native-inputs"
"$PYTHON" -B "$PKG/portable/native-v1/native_portable_grouped.py" prepare \
  --repo "$OUT/native-inputs/inputs" --core-manifest "$PKG/manifests/core-v1.json" \
  --native-root "$PKG/components/native-driver" \
  --native-manifest "$PKG/components/native-driver/artifact-manifest.json" \
  --oracle-manifest "$PKG/components/oracles/pre-execution-manifest.json" \
  --native-supplement "$PKG/components/oracles/supplements/native-diagnostics-v1/supplement-manifest.json" \
  --native-fuel-requests "$PKG/components/oracles/supplements/native-diagnostics-v1/fuel-requests.jsonl" \
  --materialization "$OUT/inputs/materialization.json" --output "$OUT/native"
"$PYTHON" -B "$PKG/portable/native-v1/native_portable_grouped.py" build \
  --prepared-manifest "$OUT/native/native-prepared.json" --profile both \
  --rustc "$RUSTC" --llvm-bin "$LLVM_BIN" --llvm-lib-dir "$LLVM_LIB_DIR" \
  --cargo-home "$CARGO_HOME" --rustup-home "$RUSTUP_HOME" \
  --receipt "$OUT/native-build-wrapper.json"
"$PYTHON" -B "$PKG/portable/make_plan.py" --kind native \
  --materialization "$OUT/inputs/materialization.json" --prepared "$OUT/native/native-prepared.json" \
  --wrapper "$PKG/portable/native-v1/native_portable_grouped.py" \
  --debug-build "$OUT/native/native-component/build-debug/verified-build.json" \
  --release-build "$OUT/native/native-component/build-release/verified-build.json" \
  --build-wrapper "$OUT/native-build-wrapper.json" \
  --receipt-root "$OUT/native-wrappers" --output "$OUT/native-plan.json"
NATIVE_PLAN_SHA=$(sha256sum "$OUT/native-plan.json" | cut -d' ' -f1)
"$PYTHON" -B "$PKG/portable/collect.py" --plan "$OUT/native-plan.json" \
  --plan-sha256 "$NATIVE_PLAN_SHA" --python "$PYTHON" --output "$OUT/native-collection" \
  --rustc "$RUSTC" --llvm-bin "$LLVM_BIN" --llvm-lib-dir "$LLVM_LIB_DIR" \
  --cargo-home "$CARGO_HOME" --rustup-home "$RUSTUP_HOME"
```

The source plan requires exactly 304 tuples. The native plan requires exactly
300 primary invocations:64 default,36 native-fuel,36 reference-fuel,144 private
driver and20 no-clobber. Expected96 source-free ELF executions are checked by
the independent comparator. Those counts describe future full CI gates, not an
extra local replay. The bounded portability proof remains four source observations
and six native invocations/four source-free ELFs.

After source-only observer preparation, prepare and build both mutation variants.
The mutation targets must remain beneath their own prepared directories because
the unchanged controller verifies that exact relationship.

```sh
for VARIANT in v2 v3; do
  "$PYTHON" -B "$PKG/portable/mutations.py" prepare --variant "$VARIANT" \
    --observer-prepared "$OUT/observer/prepared-source.json" \
    --component-root "$PKG/components/mutations/$VARIANT" \
    --oracle-manifest "$PKG/components/oracles/pre-execution-manifest.json" \
    --mutation-requests "$PKG/components/oracles/mutation-requests.json" \
    --effective-requests "$PKG/components/oracles/supplements/mutation-applicability-v1/requests.json" \
    --applicability-manifest "$PKG/components/oracles/supplements/mutation-applicability-v1/supplement-manifest.json" \
    --output "$OUT/mutations-$VARIANT"
  for PROFILE in debug release; do
    "$PYTHON" -B "$PKG/portable/build.py" \
      --prepared-manifest "$OUT/mutations-$VARIANT/prepared-mutation.json" \
      --output "$OUT/mutation-build-$VARIANT-$PROFILE" \
      --target-dir "$OUT/mutations-$VARIANT/target" \
      --profile "$PROFILE" --jobs 2 --rustc "$RUSTC" --cargo "$CARGO" \
      --rustdoc "$RUSTDOC" --cargo-home "$CARGO_HOME" --cc "$CC" --cxx "$CXX" --ar "$AR"
  done
done
"$PYTHON" -B "$PKG/portable/mutation_gate.py" make \
  --materialization "$OUT/inputs/materialization.json" --wrapper "$PKG/portable/mutations.py" \
  --v2-prepared "$OUT/mutations-v2/prepared-mutation.json" \
  --v3-prepared "$OUT/mutations-v3/prepared-mutation.json" \
  --v2-debug-build "$OUT/mutation-build-v2-debug/build.json" \
  --v2-release-build "$OUT/mutation-build-v2-release/build.json" \
  --v3-debug-build "$OUT/mutation-build-v3-debug/build.json" \
  --v3-release-build "$OUT/mutation-build-v3-release/build.json" \
  --receipt-root "$OUT/mutation-receipts" --output "$OUT/mutation-plan.json"
MUTATION_PLAN_SHA=$(sha256sum "$OUT/mutation-plan.json" | cut -d' ' -f1)
"$PYTHON" -B "$PKG/portable/mutation_gate.py" collect --plan "$OUT/mutation-plan.json" \
  --plan-sha256 "$MUTATION_PLAN_SHA" --python "$PYTHON" --output "$OUT/mutation-collection"

"$PYTHON" -B "$PKG/portable/comparison-v1/portable_compare.py" prepare \
  --component-root "$PKG/components/oracles" \
  --component-manifest "$PKG/portable/comparison-v1/component-manifest.json" \
  --materialization "$OUT/inputs/materialization.json" \
  --parser "$PKG/components/observer/parse_debug.py" --output "$OUT/comparison-view"
"$PYTHON" -B "$PKG/portable/comparison-v1/portable_compare.py" compare \
  --prepared "$OUT/comparison-view/prepared-comparison.json" \
  --plan "$OUT/source-plan.json" --plan-sha256 "$SOURCE_PLAN_SHA" \
  --output "$OUT/source-comparison"
"$PYTHON" -B "$PKG/portable/comparison-v1/portable_compare.py" compare \
  --prepared "$OUT/comparison-view/prepared-comparison.json" \
  --plan "$OUT/native-plan.json" --plan-sha256 "$NATIVE_PLAN_SHA" \
  --output "$OUT/native-comparison"
NATIVE_EVIDENCE_SHA=$(sha256sum "$OUT/native-comparison/comparison.json" | cut -d' ' -f1)
"$PYTHON" -B "$PKG/portable/comparison-v1/portable_compare.py" compare \
  --prepared "$OUT/comparison-view/prepared-comparison.json" \
  --plan "$OUT/mutation-plan.json" --plan-sha256 "$MUTATION_PLAN_SHA" \
  --native-evidence "$OUT/native-comparison/comparison.json" \
  --native-evidence-sha256 "$NATIVE_EVIDENCE_SHA" --output "$OUT/mutation-comparison"
```

Mutation collection is exactly220 internal case/profile rows. Its independent
comparison attributes eight driver results from the already completed300-row
native comparison. Baseline reuse is limited to the same profile, positive
control and exact mutation build, and the original controller re-verifies it.

Collection completion alone is never a semantic pass. Preserve `$OUT` as a
lossless CI artifact on every terminal state, including source plans, bridge
receipts, native-inputs.json, build logs, collection receipts, compressed
observations, IR/ELFs, tool/library identities and independent comparison reports.
The outer artifact index must bind the selected successor and bridge receipts to
the plans while keeping original inner core-v1 identities unchanged. Native tools
are not probed on an invocation before admission.
