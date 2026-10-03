# Reproduce source checks and interpret execution evidence

## Verify the published bytes

From the repository root:

```sh
python3 -B tests/fixtures/typed_project_unit4_parser_comparator/verify_package.py
```

The content manifest binds every published file. The verifier separately pins
the original v8b checkpoint and checks every original member. The original
checkpoint bytes, including historical status fields, remain unchanged.

## Run the synthetic controls

Materialize the already-published closed source-contract transport into a fresh
directory outside the repository:

```sh
python3 -B tests/fixtures/typed_project_unit4_contracts/transport.py extract \
  --destination /tmp/oxid-unit4-comparator-authorities-fresh
python3 -B tests/fixtures/typed_project_unit4_parser_comparator/run_source_controls.py \
  --contract-dir /tmp/oxid-unit4-comparator-authorities-fresh/workspace/shared/oxid-reset-recovery-20261003/new-contracts/parser \
  --amendment-dir tests/fixtures/typed_project_unit4_parser_location_amendment_v1
python3 -O -B tests/fixtures/typed_project_unit4_parser_comparator/run_source_controls.py \
  --contract-dir /tmp/oxid-unit4-comparator-authorities-fresh/workspace/shared/oxid-reset-recovery-20261003/new-contracts/parser \
  --amendment-dir tests/fixtures/typed_project_unit4_parser_location_amendment_v1
```

Both runs execute all 52 frozen test methods. The wrapper changes only the test
module's contract directory and amendment artifact path locator. It preserves
every test body, assertion, expected value, authority hash and comparator
function. The original source-only tests contain synthetic negative controls;
they do not run the compiler or read candidate observations. The amended
contract package and source review are independently hash-checked by the frozen
loader during positive admission controls.

The independent verifier and test wrapper are publication conveniences. Passing
these source checks does not establish candidate or hosted qualification.

## Reproduce an authorized local comparison

`evidence/local-amended-command.json` retains the exact successful command and
input identities. It used the original full-v7 debug and release manifests,
containing 248 ProjectCandidate cases and 71 OwnedCandidate relation rows each.
Their SHA256 identities are respectively:

- `16d37012d5d363f2ef5d141a75f9135018719530a19cefa1a12be8f2537cf1c3`
- `f93546f405265d0e3cfdec439d648bb9acbe7d78e1012baeff4710ac4ffd4b4f`

The exact local authorization, SHA256
`256fa85fec4a55d7fe6644d409452ff96c3b0f60b3e457270381e3b1be55099d`,
is retained separately. Its filename version is v3; its protocol schema is
`oxid-unit4-parser-comparison-authorization-v2`. The authorization names the
original local checkpoint paths and must not be silently rebased or relabeled.
Re-running requires the complete verified raw evidence, build/binary/compiler
and helper identity chains, and a newly issued exact authorization if any
artifact path or executable context changes. Those large execution artifacts
are deliberately not duplicated in this source checkpoint.

The CLI shape remains:

```sh
python3 -B /approved/comparator/comparator.py \
  --contract-dir /verified/base-parser-contract \
  --authorization /approved/authorization.json \
  --authorization-sha256 EXTERNALLY_APPROVED_SHA256 \
  --execution-manifest /verified/debug/execution-manifest.json \
  --execution-manifest /verified/release/execution-manifest.json \
  --output /fresh/comparison.json
```

Authorization must bind the executing checkpoint, original source/build/helper
authorities and all four amendment authority artifacts. The comparator rejects
missing, stale, duplicate, zero-execution and unsupported-platform evidence.
The result reports original execution identities and the distinct effective
amended comparison identity. The prior failed result remains a separate record.

## Interface for fresh portable execution

A portable adapter can pin the complete bytes of
`frozen/v8b/comparator.py`, `debug_decoder.py` and `test_mutations.py`. The current
portable adapter uses its own `frozen/comparator/` layout with equal bytes; this
publication does not change that bound layout or its source/recipe authority.
The reviewed semantic entrypoints include `load_contract`, `check_observation`,
`compare_rows`, `admit_contract_amendment` and `compare_effective_rows`.

Fresh-build admission is separate from this comparator's historical local
receipt admission. Any replacement must bind actual source/patch/helper bytes,
Cargo profile/target/toolchain/binary, measured runtime host and complete raw
case receipts, and preserve the reviewed semantic definitions. No source
manifest, receipt summary or synthetic suite may substitute for fresh actual
debug/release observations. This package installs no hosted job and claims no
fresh portable or hosted pass.
