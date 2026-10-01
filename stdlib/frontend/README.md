# Oxid frontend demonstrations

These `.ox` modules explore frontend APIs and formatting. The production lexer, parser, and binary OXBC writer are still implemented in Rust.

- `lexer.ox`, `parser.ox`, `ast.ox`, and `recovery.ox` provide preview helpers.
- `diagnostics.ox`, `modules.ox`, `syntax.ox`, and `lint.ox` build descriptive output for the demos.
- `pipeline.ox` composes the preview stages.
- `emit.ox` describes emission plans.
- `bytecode.ox` serializes instruction records as text, which `compiler/main.ox` exercises.

The textual emitter's output is different from the binary `.oxb` format used by `oxid compile`. These modules are not a complete compiler or the production frontend.

See [frontend status](../../docs/FRONTEND.md), [architecture](../../docs/ARCHITECTURE.md), and [self-hosting requirements](../../docs/SELF_HOSTING.md).
