# Self-compiling command compatibility

`oxid self-compile`, `oxid emit`, and `oxid self-host` are retained aliases for the
current [artifact round-trip](BOOTSTRAP.md) implementation. They do not compile
the compiler using an independently implemented Oxid compiler.

`compiler/main.ox` checks a representative textual emitter sequence from
`stdlib/frontend/bytecode.ox`. `compiler/providers.toml` records planned ownership,
but does not dispatch the production frontend or binary artifact writer. Its
parity declarations are checked as text; they do not enforce independent provider
execution or prevent fallback.

Future replacement requires real provider APIs, execution evidence, no-fallback
checks, and [compiler rebuild/fixed-point verification](SELF_HOSTING.md). Keep the
current aliases and recovery implementation compatible while that work proceeds.
