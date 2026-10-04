# Fixed-array source typing replay: independent review

PASS for the public replay package whose `freeze-manifest.json` has SHA256 `977860da7912737196a4dcf027000f58afcbe03e22f9d9ae26bb2fd60f1fa85d`.

The package is intended for `tests/fixtures/fixed_array_source_unit3/typing-replay-v1`. This statement can be supplied to its `replay.py --review` argument. The result applies to these exact package bytes and the bounded source-resolution/type-checking scope below.

## Reviewed identities

- Replay driver SHA256: `8042453291886a6e44e7754bf36ef1a9435b926ca8511161f838b0d73be23d43`.
- Comparison predicates SHA256: `1c96f181be1dc92889b3738374954376ff93c7bcbff9181fa6d1b0dc2298157a`.
- Published input manifest SHA256: `2cba1dbd200d75fbeb33b504b08279d89d2baa1784f88718413305446d458c35`.
- Historical semantic-authority manifest SHA256: `7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039`.
- Reconstructed base commit: `2e84c9de9d9b02b62283fff1902cf1a840254ac4`; base tree: `86b90e5cb5eaf03a81f9faaa3b4f494248d2c60c`.
- Full candidate source-closure manifest SHA256: `05a280a589493db7c22f32eb6c98af0054eb28bf6e480cea751ac4cfb9d3c993`.

The wrapper reconstructs the pinned base, applies the packaged patch, and checks all 1,297 source files. It verifies the published input package, materializes 65 source bodies across 54 cases, checks nine exact semantic projections from the historical authority, and creates source-only requests. It does not claim complete historical-package membership. Output paths inside the replay package or input repository are rejected before output creation, including containment through symlink parents. Integrity and execution gates remain active under optimized Python.

## Actual execution and independent verification

Two fresh staging directories completed the combined verify, materialize, request, observe, and compare flow on their first attempt, one for each profile. Each profile produced 54 observations: 22 typed successes and 32 complete diagnostic outcomes. The independent review checked all source and fixture bytes, projection and request identities, binary copies, execution and artifact receipts, and the exact one-test execution witness. Independently rerunning the unchanged comparison predicates over all 108 artifacts reproduced the reports.

Both executions used exact previously reviewed binaries:

- Debug binary SHA256: `61856cf6f02919e7b42826e1d84e52a02ae4ba16b9fb8cd550a20e3cb3eeb24a`; result SHA256: `5ed7ce4947d816a7cc8740fbcfc384bf611299d7418dda4d09d1fe22e0eb1819`; run receipt SHA256: `2ad83207c951df53ec46ae147fd27ce2458e3bb87d519943ada0d4fba0e9a80a`.
- Release binary SHA256: `682d129230c3a8b8605cb1b7f81198449ce4973be9b014cfcfd03042634b6645`; result SHA256: `119dc249a7368dfd6f782c29002bef62eaa8803ca86a39560986fad75f15baa0`; run receipt SHA256: `a9446206b2e38ecf8ccd9a9562d212dfb3fe9fbdf86aa0d3befdac7825afa531`.

These executions establish fresh source/materialization/observation replay with verified binary reuse. They do not establish a fresh compiler build. Case identity, outcome, row count, and semantic-unit count agree with the original qualified observations. Absolute fixture paths differ between fresh stages, so raw profile artifact byte equality is not asserted.

## Scope

This result covers the 54 source-resolution/type-checking observations and their replay packaging. Separate resource, fence, and sensitivity controls are not rerun or independently credited by this wrapper. The two RHS snapshot effect cases remain deferred. Lowering, ownership, runtime, native execution, fuel, Q/pilot, public activation, CI, and execution at an unverified later head are outside this result. Public parser and production raw-array gates remain closed.
