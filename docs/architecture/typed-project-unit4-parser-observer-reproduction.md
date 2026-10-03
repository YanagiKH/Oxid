# Reproducing the Unit4 parser observer in CI

The published helper package and the compiler input checkout are separate authorities. The helper package is additive at `tests/fixtures/typed_project_unit4_parser_observer/`. Its `prepare.py` intentionally requires compiler checkout HEAD exactly `d9e6b9bf172abd5e15da7212c9e6224e29ccc768`, and verifies each copied tracked file against that commit's Git object. Do not change that pin to the current PR head or the merge checkout.

A default shallow `actions/checkout` with `fetch-depth: 1` is insufficient unless that historical object happens to exist. The combined CI job must provide the exact historical commit before calling the helper. Use either full history (`fetch-depth: 0`) or an explicit authenticated fetch of that exact commit from this repository, then verify it is a commit object. No fallback to the current source is permitted.

Example setup after checkout:

```bash
set -eu
OXID_PARSER_AUTHORITY=d9e6b9bf172abd5e15da7212c9e6224e29ccc768
# Omit this fetch only when checkout supplied full history and cat-file succeeds.
git fetch --no-tags origin "$OXID_PARSER_AUTHORITY"
git cat-file -e "${OXID_PARSER_AUTHORITY}^{commit}"
git worktree add --detach "$RUNNER_TEMP/oxid-unit4-authority" "$OXID_PARSER_AUTHORITY"
python3 "$GITHUB_WORKSPACE/tests/fixtures/typed_project_unit4_parser_observer/prepare.py" \
  --repo "$RUNNER_TEMP/oxid-unit4-authority" \
  --output "$RUNNER_TEMP/oxid-unit4-instrumented"
```

Use a separate fresh output directory for each derived view. The control view uses the same authority checkout and `--control`. Do not modify the pinned authority worktree or reuse an output directory from a previous run. The resulting candidate source manifest must retain SHA256 `2e96ea16fe01532366a4af1d69f4fc8c18dfb2cca305da131aced3c9565b167a` for this exact input set.

Use the recorded official Rust 1.99.0 toolchain and explicit `x86_64-unknown-linux-gnu` target. `build.py` records the compiler binary/version, target, profile, source overlay and resulting executable, uses two jobs, and disables incremental compilation. Both debug and release are required. Run Python with assertions enabled: `-O`, `-OO` and a nonempty optimization setting that enables optimized execution are deliberately rejected by prepare/build/run/passivity before work.

Before frozen-corpus collection, provide the durable exact contract checkpoint and the independently reviewed observer/comparator identities. Preserve the helper-only v7 rebind when reusing the existing v4 compiled observer: historical build receipts and the compiled observer source identity remain unchanged; the new guarded collection helper must be admitted explicitly by its reviewed hash. Never relabel an old binary as newly built.

The normal qualification roster is 248 source cases and 319 mode observations per profile, 638 observations across both profiles. Collection success and ordinary passivity are intermediate results. Only the independent comparator can qualify the complete evidence. Missing historical Git objects, toolchains, profiles, source identities, execution rows or required observations must fail the job rather than skip a gate.

These instructions document prerequisites for the combined CI integration. They do not claim that the combined CI gate has run or that the full parser corpus has passed.

## Explicit admission controls and measured host

The v6 collector separately records `platform.uname()` OS/machine evidence and native pointer width before launching any candidate process. It requires the measured Linux/x86_64/64 tuple to agree with the raw Rust process ABI and explicit compiler target. Compile-target labels alone are insufficient.

When running the Python control suite, supply the independently materialized, frozen parser contract directory through `UNIT4_HOST_CONTROL_CONTRACT`. The host rejection process-control test honestly reports skipped if that input is absent. A complete CI admission-control run must include all three unsupported-host controls and all eight optimized-Python controls, with no skipped host test:

```bash
UNIT4_HOST_CONTROL_CONTRACT="$OXID_PARSER_FROZEN_CONTRACT" \
UNIT4_HOST_CONTROL_REPORT="$RUNNER_TEMP/parser-host-controls.json" \
UNIT4_OPT_CONTROL_REPORT="$RUNNER_TEMP/parser-optimization-controls.json" \
python3 -B -m unittest discover \
  -s "$GITHUB_WORKSPACE/tests/fixtures/typed_project_unit4_parser_observer" \
  -p 'test_*.py' -v
```

`OXID_PARSER_FROZEN_CONTRACT` must name the exact reviewed contract package after its transport/identity validation, not regenerated expectations. Preserve both control reports, test stdout/stderr, the exact helper-only rebind and its independent review. Existing v4 executable receipts remain bound to their actual historical compiled identity; the comparator must explicitly admit the exact reviewed v7 helper hashes. Neither source identity nor raw observations may be relabeled silently.

## Linear mechanical Debug decoding

The v7 helper preserves the v6 driver/schema and changes only three mechanical Debug-decoder regex operations to precompiled pattern matching at the existing cursor. This avoids repeated suffix copying on the100,000-token successful tape. Do not substitute a different AST projection or omit tokens to speed collection.

Independent review verified75 old/new value/error controls and complete equality with the retained8.94MB old Debug result; the new decode took about2–3seconds in those controls. This measures observer normalization overhead, not compiler performance. The exact normalizer SHA256 is `89c77305d8b51bebbe71b06c8e3b2d82a09ae76a05f53eadae5e2820d48b2822`. Bind its reviewed helper-only rebind when reusing the unchanged historical Rust binaries, or produce fresh CI build/recipe evidence with actual paths and binary identities.

Complete collection has produced248cases/319mode observations per profile, but collection is not qualification. The first full frozen comparison reported three distinct predicate/contract-coordinate mismatches in both profiles, with no collection or production defect established. Preserve that result and all raw evidence; any comparator repair or source-contract correction requires a separate reviewed identity before another qualification comparison. These reproduction instructions make no full-pass claim.
