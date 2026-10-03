# Public v3 source-coordinate amendment v1

This package corrects one frozen diagnostic location. It does not change source bytes, diagnostic code/stage/message, operation or host selection, later-diagnostic projection scope, lifecycle, or any inherited Unit1/2/3 expectations. The original public v3 package and transport remain immutable.

The source `mod child; fn pub()->i32{return 1;}` has its unique reserved function-name token `pub` at UTF-8 bytes `[14,17)`, line 1 scalar columns `[15,18)`. The old span `[13,16)` selects ` pu`. The other two first-diagnostic-only roots were audited: `host-old-reserved-name` is correct at `[3,6)`, line 1 columns `[4,7)`; `host-malformed-signature-first` is correct at `[20,21)`, line 2 columns `[10,11)` for the first colon after the parameter list.

## Identities and admission

- Base public v3 freeze: `12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85`
- Exact amendment: `e4c3b886ec2d0cf6448a1fc1d4f15bac5ee081d05e80876b13055587a421974d`
- Effective contract descriptor: `68c310e4b0d1f63b3cab25839dade5f4cdfa55cecf752409314dba7311ef2eb7`
- Effective public document, canonical JSON: `bb31b5b4ddd5467170ade55e7cc81f92378570f860ff17bfa86d821feab651ec`

The effective authority is `oxid-unit4-public-v3-location-amendment-v1`. Its exact descriptor is `artifacts/effective-contract.json`; `receipt-identity.json` supplies the mandatory observation identity values. The old document's identity metadata remains historical base metadata. It cannot substitute for the effective descriptor or authorize labeling new comparisons as unmodified v3. Existing failed or successful v3 records retain their original identities and bytes.

Before application, an adapter must verify the approved transport and all original closure bindings, then independently pin both this amendment and the effective descriptor. It must check the original raw public-document hash, the exact case ID and index, whole-case and diagnostic-subtree hashes, source hash and old location. It applies the sole replacement in memory once. It then verifies the new subtree hashes and the complete canonical effective-document hash. Only `start`, `end`, `scalar_column`, and `end_scalar_column` change. Additional or repeated application, mismatched base/source, stale identities and unspecified changes must fail closed. Receipt identity fields are additional to existing compiler, profile, host, input closure and observation inventory bindings.

No expanded full corpus or rewritten v3 file is necessary. All unchanged companion authorities stay bound to the base freeze. Application does not rerun the historical v3 generator, whose hard-coded old coordinates are preserved as provenance. The source-only generator here is a narrow reproducible derivation, not a candidate-output converter.

## Reproduce the three artifacts

Use the approved original package root or its verified materialized equivalent. The destination must be fresh. This command only reads the frozen public contract and three root sources; it does not invoke the compiler.

```sh
python3 -B build_amendment.py \
  --base-root /verified/materialization/workspace/shared/oxid-reset-recovery-20261003/new-contracts/public-v3 \
  --output-dir /tmp/oxid-public-coordinate-proof-fresh
```

The output must match the exact amendment, effective descriptor and source-coordinate proof hashes in `AMENDMENT-CHECKPOINT-v1.json`. Canonical object hashing is explicitly defined in those artifacts; artifact-file hashes include their final newline. The proof inventories every byte of all three small root sources and enumerates the four changed JSON leaf pointers.

This package contains source-authority artifacts and source-only checks. Compiler qualification and independent adoption review are separate.
