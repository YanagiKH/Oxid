# Baseline validation record

Date: 2026-10-01 UTC. Source baseline:
`7e681d25fa58828b817d66a8701a6913a7158932` (source before these baseline changes).

## Environment

- Linux x86_64; Rust host `x86_64-unknown-linux-gnu`
- rustc 1.98.1 (`48a229cea`, 2026-09-01), Cargo 1.98.1 (`797e8a9bc`)
- Rust toolchain LLVM 22.1.8; this is the host Rust compiler's LLVM, not an Oxid backend
- GCC/G++ 14.2.0, Python 3.12.14
- Dependency resolution used the committed Cargo.lock

## Unchanged baseline

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-targets --all-features --locked` | Passed: 44 tests |
| `cargo build --release --locked` | Passed |
| `python3 scripts/verify_repo.py target/release/oxid` | Passed: 121 source checks, 67 runnable programs, repository checks |
| `target/release/oxid bootstrap --check` | Passed: artifact serialization round-trip |

## Added characterization

`cargo test --test legacy_semantics --locked` passed all eight legacy tests against
the unchanged runtime semantics. The cases cover f64 arithmetic/epsilon equality,
shared containers, logical short-circuiting, evaluation order, lazy task order and
memoization, five runtime-error reasons with source locations, and module
initialization, including a known source/artifact ordering difference. Every case executes from source and from an artifact produced by
the production writer. This is partial legacy regression coverage, not a proof
of complete backend equivalence.

The ordering test uses separate expected outputs for source and artifact
execution; the other seven tests expect matching results. A numeric-equality
mutation was also checked: changing epsilon equality to exact equality made the
new regression test fail, and restoring the implementation made it pass.

## Integrated verification

An independent rerun on the Linux environment above passed:

| Check | Result |
|---|---|
| Full Rust test suite | 53 tests: 44 existing, eight legacy behavior, one generated-documentation test |
| Python feature-status validator tests | Seven passed |
| Feature inventory validation | 12 entries passed metadata checks |
| Formatting and Clippy | Passed, with warnings denied |
| Optimized build | Passed |
| Repository verifier | 121 source checks and 67 runnable programs, plus README parity, local links, SVG XML, project build, and doctor |
| Documentation examples | Nine Oxid code blocks executed successfully; documented output matched |
| New-project quickstart | Source run, build, artifact run, and tests passed |
| Local heading links | Passed |
| Documentation diagrams | Four SVGs rendered and visually checked |
| Patch whitespace | `git diff --check` passed |

Benchmark report schema checks and source installation also passed. The generated
API reference has a CLI regression test covering its descriptions of artifacts,
lazy tasks, and legacy bootstrap commands. These results cover the tested local
environment; they do not replace the configured cross-platform CI runs.

## Not demonstrated here

- Windows, macOS, Linux ARM64, cross-host artifact comparison or native cross-compilation
- Docker build/run (Docker is not installed in this environment)
- GPU/MCU hardware, native AI training, Rust performance parity or production pilots
- Rust-level static memory safety, no-fallback providers or true compiler self-rebuild
- Reproducible native payloads, release provenance/signatures or v1.0 release readiness

The configured CI matrix remains unchanged. M0 is still in progress: full
specifications, ownership/review assignments, threat model, corpora, and the
remaining roadmap acceptance evidence remain outside this baseline.
