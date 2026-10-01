# Frontend

The Rust frontend handles lexing, parsing, AST construction, source diagnostics, module resolution, and binary AST serialization. `oxid lint` runs source syntax checks; it is not a static type or ownership checker.

Every parsed top-level statement carries a `SourceSpan` containing the source name and start/end line and column. Imported files keep their own source identity during execution and after OXBC serialization, so failures can point to the module that produced them instead of only the entry file.

## Provider declarations and planned replacement

`compiler/providers.toml` declares component ownership, and `oxid frontend`
reports those declarations. It does not select the execution path: the production
parser and binary OXBC writer are Rust implementations. The Oxid-authored emitter
currently emits textual instructions in a separate demonstration.

`oxid bootstrap --check` verifies required manifest text and artifact
serialization round-trip. Production provider dispatch, traceability, and a
no-fallback mode still need implementation before provider replacement can be
validated. See [the implementation baseline](architecture/current-baseline.md).
