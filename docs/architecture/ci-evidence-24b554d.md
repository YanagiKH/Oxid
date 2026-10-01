# CI evidence for the first M0 baseline

The [Repository CI run 108](https://github.com/YanagiKH/Oxid/actions/runs/36807652559)
completed successfully on 2026-10-01 for
[PR #3](https://github.com/YanagiKH/Oxid/pull/3).

## Revision identity

- PR head: `24b554de13e5cd03f8191cf85ccbdb0c0249ba1b`
- Base: `7e681d25fa58828b817d66a8701a6913a7158932`
- GitHub's tested merge revision: `577c4180eb75a13fc2dab5b53fb85fe2f98c2d02`
- Head and tested merge tree: `c6a7773a65b88293c3d5b08f40700b73f874b3cd`

The identical trees establish that the CI merge checkout contains the reviewed
head content against that base. This record covers that snapshot; it does not
validate later commits or the additional module corpus introduced afterward.

## Completed jobs

| Job | Result |
| --- | --- |
| Full repository verification | Passed, including formatting, Clippy, Rust/native tests, feature-status tests, release build, repository verification, source install, Docker build and container smoke test |
| Linux x86_64 runtime | Passed |
| Windows x86_64 runtime | Passed |
| macOS x86_64 runtime | Passed |
| macOS ARM64 runtime | Passed |
| Workflow and metadata audit | Passed, including actionlint |
| Cross-platform serialized-AST artifact equivalence | Passed |

The runtime jobs ran the Rust/integration tests, Oxid tests, syntax example,
bundle build, version check, artifact round-trip and bounded benchmark/schema
checks. The snapshot contains 53 Rust tests and seven Python metadata tests.
The corresponding local evidence is in [baseline validation](baseline-validation.md).

## Artifact evidence

The [equivalence job](https://github.com/YanagiKH/Oxid/actions/runs/36807652559/job/110195734613)
reported four byte-identical `compiler.oxb` artifacts:

- Size: 12,391 bytes each
- SHA-256: `b89230fd305211c21275519f601121f76c1ee0caf32a9b73608ff87163f1c68a`

These are serialized compiler-demonstration AST artifacts, not independently
rebuilt native compilers. The workflow retains uploaded artifacts for seven
days; this source record preserves the revision, command outcome and payload
digest rather than promising indefinite artifact availability.

## Remaining limits

Linux ARM64, native Oxid compilation, static ownership safety, real self-hosting,
GPU/MCU hardware, native AI training and production pilots are not validated by
this run. Three-iteration benchmark checks validate the harness/schema, not a
performance comparison with Rust. CI success does not certify M0 completion or
v1.0 readiness.
