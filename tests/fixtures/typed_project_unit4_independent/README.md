# Typed-project activation validation inputs

This checkpoint adds the public CLI comparison helpers, logical lifecycle observer, source-only lifecycle amendment and retained expectation generators for typed-project activation. The compiler and integration-test patch matches its earlier 28,881-byte identity exactly. Fresh formatting, strict Clippy and ordinary Rust suites pass; each profile executes 789 tests and leaves 21 explicitly ignored gates separate.

[input-checkpoint.json](input-checkpoint.json) binds every payload and distinguishes historical digest matches, source recovered without an earlier standalone digest, and the newly reconstructed 15-case lifecycle subset. The compressed subset preserves its separately recorded decoded bytes and SHA-256. Presence here does not establish a new corpus execution.

The corrected predecessor comparator is `components/public/replay_predecessors_v2.py`; the superseded version is retained under `history/`. These helpers retain their original configured paths and are consumed through a path adapter. Complete public/parser contract tables, adapter binding and combined collection/comparison are the next integration step. Earlier raw qualification artifacts are unavailable after the workspace replacement.

Ordinary checks use official Rust 1.99.0: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo test --all-targets --all-features --locked` in debug and release. Native qualification uses LLVM 19.1.7 and remains a separate gate.
