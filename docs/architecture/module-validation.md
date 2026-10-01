# Module corpus validation

Date: 2026-10-01 UTC. Source reference:
`24b554de13e5cd03f8191cf85ccbdb0c0249ba1b`.
This increment adds tests and documentation; it changes no production runtime
code and does not choose new language semantics.

## Local results

Environment: Linux x86_64, rustc/Cargo 1.98.1, GCC/G++ 14.2.0, Python 3.12.14.
Dependencies used the existing Cargo.lock.

| Check | Result |
| --- | --- |
| `cargo test --test legacy_modules --locked` | Passed: eight new cases |
| `cargo test --all-targets --all-features --locked` | Passed: 61 tests total (44 unit, one generated-documentation, eight legacy-semantics, eight module cases) |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo build --release --locked` | Passed |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | Passed: seven tests |
| `python3 scripts/verify_repo.py target/release/oxid` | Passed: 13 status entries, 121 source checks and 67 runnable programs |
| `git diff --check` | Passed |

## Scope and limits

See [the module contract](../../spec/legacy-modules.md) for each case's distinct
source/compile/artifact expectation. Missing/cyclic imports and imported runtime
errors check reasons and available locations rather than platform-specific OS
text. Missing/cyclic-import compilation failures create no final artifact.
Runtime source-dependency tests copy an artifact into an isolated directory without deleting source files.

The new corpus has only been executed locally on Linux x86_64 at this point.
[The earlier CI record](ci-evidence-24b554d.md) covers the previous snapshot and
must not be used to claim cross-platform validation of these new cases.

No production performance, memory, safety or migration claim changes in this
increment. Tests add development/CI work only. Broader module semantics, static
name resolution, runtime-import policy, and artifact relocation remain open
specification work; this is not completion of M0.
