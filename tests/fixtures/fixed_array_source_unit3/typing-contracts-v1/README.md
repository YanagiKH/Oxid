# Private array source 3B1 replay inputs

This is the scoped public replay-input assembly of the independently admitted prospective authority. It preserves every semantic/control JSON file and the observer/resource/comparison requirements byte for byte. It adds ten source bodies across seven small cases. The other 55 source bodies and 15 inspected source snapshots are referenced by exact existing Git blobs, not copied or changed.

The historical 94-member authority manifest has SHA-256 `7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039`. Its source-contract admission report has SHA-256 `0d70789953ebb1ac6ad05a453bd5428fe9c17b8f6ce17153edcd5e56ac3cde48`. Both bind selected base `38922724d32d849f4f46114bde137e208602fdac`, tree `d4c5cfac14f3fb250e6c00e5b1777365e8bf85a8`. The original manifest is included only as historical lineage evidence. This directory does not contain or claim to reconstruct all 94 historical members.

`freeze-manifest.json` defines this package's complete scoped input closure: every shipped file and every required external base-object input, with byte size/SHA-256 and Git identity. `lineage.json` maps exact copied payload members to the admitted authority and lists every historical member not shipped. Path-dependent derivation instructions, the historical authoring script, private provenance and old static verification artifacts are deliberately excluded. No historical absolute path is used by this package.

Run the scoped verifier with an explicit local Git repository containing the selected base:

    python tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/verify-inputs.py --repo .

It verifies every payload member, external blob, source identity and unchanged 29-diagnostic roster before reporting success. Optional `--materialize-sources NEW_DIRECTORY` copies all 65 verified source bodies into 54 case directories for the source-only observer. It refuses to overwrite an existing output directory. The verifier performs no build, compiler observation, production mutation, network access or remote operation.

Packaging revision 2 corrects a verifier defect found in revision 1: Python optimization removed assertion-based integrity checks. All qualification checks now raise explicit runtime errors and remain active under `python -O`. Changed, missing and malformed input controls were checked in both modes. The earlier package is preserved as failure history; all semantic/control expectations and source bytes remain unchanged. The active closure contains 23 listed file bodies plus this package's freeze manifest, 24 ordinary files total, and 75 external Git inputs.

The new cases are guard-empty, guard-record-only, guard-array-free, guard-project-root-eof, reference-access-modes, reserve-across-modules and same-signature-callee. Their source body paths/hashes are listed in the manifest and source roster; guard-empty is intentionally zero bytes. Existing source cases remain unchanged references under contracts-v2. The element-boundary supplement remains an unchanged prerequisite.

Admission of these expectations is not candidate implementation or adapter acceptance. The independent adapter mapping must be reviewed before observations; expected JSON never enters the compiler. Read OBSERVER.md, RESOURCE.md and COMPARISON.md for the boundary. The two rhs-snapshot effect cases remain deferred. No lowering, ownership, runtime/fuel, pilot or array-gate activation is qualified here.
