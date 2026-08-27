# Frontend

The active frontend stages are lexing, parsing, AST construction, source diagnostics, module resolution, emission, and linting.

Every parsed top-level statement carries a `SourceSpan` containing the source name and start/end line and column. Imported files keep their own source identity during execution and after OXBC serialization, so failures can point to the module that produced them instead of only the entry file.

## Incremental providers

`compiler/providers.toml` records the implementation selected for each replaceable component. Oxid 0.9 selects the Oxid-authored emitter; lexer, parser, diagnostics, and modules remain stage-0. `oxid frontend` prints the effective manifest, and `oxid bootstrap --check` verifies its required parity policy without writing artifacts.

This provider boundary permits one component to move to Oxid only after its behavior and deterministic outputs are checked, while keeping a working recovery bootstrap.

