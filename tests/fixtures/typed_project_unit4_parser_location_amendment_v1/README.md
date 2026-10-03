# Parser v1 source-coordinate amendment v1

One first-diagnostic primary in the frozen parser corpus identifies the wrong three bytes. Case `public-root-module-before-reserved-function` has source `mod child; fn pub()->i32{return 1;}`. Its unique reserved function-name token `pub` is at UTF-8 bytes `[14,17)`, line 1 scalar columns `[15,18)`. The old `[13,16)` span selects ` pu`.

Only four integers in case index 246 change: `start` 13 to 14, `end` 16 to 17, `column` 14 to 15, and `end_column` 17 to 18. The other 247 cases, every source byte, diagnostic message/code/stage, recognition expectation, limits, roster, profile and platform scope remain identical. No broad recovery-sequence claim is added.

All three `public-root-first-error` cases were rechecked from their exact embedded source bytes:

- `public-root-original-reserved-function`: unique `pub` at `[3,6)`, line 1 columns `[4,7)`; existing projection correct
- `public-root-module-before-reserved-function`: unique `pub` at `[14,17)`, line 1 columns `[15,18)`; amended
- `public-root-module-before-signature-error`: first colon after `()` at `[20,21)`, line 2 columns `[10,11)`; existing projection correct

## Authority identities

- Base package freeze: `7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027`
- Base decoded corpus: `b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc`
- Amendment: `70b7228c3b3c3cec074dc10a84e8e338ed9b443f6b95b1be883b9a43b225e451`
- Effective descriptor: `9bf8a80cbb30c384a70fd5696e19428d2b0aad535740e7cbe2ce8992ef96f020`

The distinct effective identity is `oxid-unit4-parser-v1-location-amendment-v1`. Exact files are in `artifacts/`; `receipt-identity.json` specifies all mandatory effective comparison identity values. These fields supplement existing raw observer, build, host, profile and case inventory bindings. Historical raw execution records and the original document's internal `contract_id` remain base-contract identities. They must not be mislabeled as fresh comparisons under the effective authority. A new explicitly authorized comparison may consume unchanged original raw observations after verifying exact source, seam, profile and observer identities; it reports both the original execution-contract identity and this distinct effective comparison-contract identity. Existing observations alone, or relabeled receipts, do not establish amended qualification.

Admission must independently pin both amendment and descriptor. Verify the original package and transported source-authority closure first. Then bind the compressed and decoded corpus, case index and ID, source, full old case and diagnostic hashes, complete old diagnostic subtree, and old primary. Replace the sole specified primary in memory once. Verify the complete new diagnostic, new hashes, exact four integer changes, and full canonical effective-document hash. Any wrong old value, additional change, stale authority or second application must reject. `artifacts/effective-contract.json` defines canonical serialization and the complete hash requirements.

The frozen gzip, generator, validator, original freeze and original transport remain byte-identical. Do not regenerate the historical corpus to apply this amendment. A separate effective descriptor carries the correction without duplicating inherited data or rewriting old observations.

## Earlier location audit

The earlier 533-location audit was a coordinate-consistency audit. `new-contract-review/audit_parser_source_only.py` lines 42–56 checked file ownership, in-range offsets, UTF-8 prefix decoding, and line/scalar-column values recomputed from the already-supplied offsets. Its source SHA256 is `6e6dc8a6d944f7fa5975621a1a14603ac722401f5c61fe9bba85720f4abd76ed`. The frozen `validate_source_contract.py` lines 36–40 and 64–66 made the same check. Neither routine independently selected the offending token for these three roots.

Thus `[13,16)` and columns `[14,17)` passed because they describe the same wrong substring consistently. Separate token-semantic checks in that audit covered Q35 and diagnostic-cap rows, but did not cover these three roots. The count 533 must not be interpreted as 533 independently established offending-token origins. The present proof first locates each root's unique lexical target from source, then derives its coordinates; it preserves the earlier audit reports as historical records.

## Reproduce

Use an approved original parser package root or its verified materialized equivalent, and a fresh output directory:

```sh
python3 -B build_amendment.py \
  --base-root /verified/materialization/workspace/shared/oxid-reset-recovery-20261003/new-contracts/parser \
  --output-dir /tmp/oxid-parser-coordinate-proof-fresh
```

The generator verifies all 11 frozen package members before decoding the exact pinned corpus. It emits only the amendment, effective descriptor and three-source coordinate proof. It does not invoke the compiler or read candidate observations. Artifact hashes include their final newline; canonical subtree/document hashes do not. `AMENDMENT-CHECKPOINT-v1.json` binds this small package. Independent adoption review and candidate qualification are separate steps.

Suggested publication location: `tests/fixtures/typed_project_unit4_parser_location_amendment_v1`, alongside the unchanged closed transport package.

The first provisional descriptor (SHA256 `8d3bf7cdb027e1c9589275787dafbd8c6b89439521f51201dcc47571cd014c72`) is retained under `history/provisional-v1/`. Its successor clarifies reuse of unchanged raw observations for a newly authorized comparison. Amendment and source-proof bytes did not change. Historical material is excluded from the active publication file list.
