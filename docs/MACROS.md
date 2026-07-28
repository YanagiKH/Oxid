# Macros and compile-time behavior

Oxid currently supports source preprocessing macros.

- `macro name(a, b) => body;`
- expansion runs before lexing and parsing
- the preprocessor is cache-backed under `.oxid/cache`
- the feature is intentionally small so it can evolve into a token-level system later
