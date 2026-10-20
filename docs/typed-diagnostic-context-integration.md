# Typed source diagnostic context: CLI integration

## Integration status

This combined source includes the reviewed `frontend::diagnostic_context`
renderer, its module declaration, and the driver integration below, alongside
the manifest-selected project formatter. The renderer's temporary
non-test dead-code allowance is removed because the driver now calls it.
The final combined tree has not been compiled or tested in this workspace.
Separate renderer and formatter validation does not validate this integration.

The renderer's integration contract is to retain the existing human diagnostic
as a prefix and append source context. The 100-display-cell excerpt limit is
a display bound, not a whole-line allocation bound: the renderer processes the
source line before choosing its display window, under existing source-size
admission limits. A successor-only correction gives zero-width spans at tabs
and escaped controls a single caret; its added regression test is not yet run.

## Deliberately narrow selection

Source context is requested only for text failures from:

- Ordinary typed source `check` and Result-entry `compile --backend=llvm`.
- Explicit manifest `check` and Result-entry `build`.

A diagnostic must have a primary source span and a stage in the closed set
`source`, `lex`, `parse`, `resolve`, `type`, or `ownership`. Internal `E0500`
diagnostics are excluded even when their stage is in that set. Unknown future
stages, native admission/emission/toolchain errors, compiler-session allocation
errors, and publication errors retain their old text.

The allowlist is needed because manifest `session.build()` returns both source
checking and later native/publication failures through the same result. The
session is not changed or checked twice. The underlying stage names are existing
frontend names: source loading uses `source`, lexer errors use `lex`, parser errors
use `parse`, name resolution uses `resolve`, type mismatch errors use `type`, and
owned-source diagnostics use `ownership`.

## Compatibility boundary

The original `report()` body is unchanged. The explicit source-error wrapper
falls back to it for JSON, successes, disabled context, and summary shapes other
than failed check/compile. No new CLI flag, record field, status code, or summary
is introduced. JSON still calls the original `Diagnostic::render_json` and uses
the original summary literals and output channels.

All `run` routes keep their original text, including source errors found before
execution and errors produced during reference execution. All Process-entry
routes remain unchanged, including Process compile/build. Native run, project
test/package/format, experimental lexical/import/producer routes, and their
existing reporting contracts are outside this change.

There are no changes to `diagnostic.rs`, source owners, compiler session logic,
OIR, native code, transport/cost-model definitions, or formatter source. Source
loading, checking, native emission, publication order, and success paths are not
changed. This is presentation routing only, not a new source/compiler feature.

## Focused validation to run on the combined source

Run on the exact combined tree with an already installed toolchain and cached
dependencies:

```sh
cargo test --locked --offline --bin oxid frontend::driver::diagnostic_context_tests
cargo test --locked --offline --test typed_diagnostic_context
```

The four unit controls cover opt-in selection, existing frontend stages, missing
primary spans, internal/backend/runtime/experimental/future-stage fallback,
Result-versus-Process build text, and byte-exact build JSON.

The six Linux x86_64 CLI controls cover syntax and type errors in source check/compile and
manifest check/build; exact diagnostic JSON and summary records; unchanged
Result run source and runtime errors; unchanged Process compile/build/run and
native-run pre-launch source errors on Linux x86_64. Native candidates are invalid
before emission and use a nonexistent LLVM directory. The tests do not execute
LLVM, a native artifact, an experimental provider, or qualification fixtures.

Every fixture checks that source and a pre-existing requested output remain
unchanged and that no cache, managed output, lock, or staging entry remains.
Commands have a ten-second subprocess deadline with both output pipes drained.

Recheck the renderer's eleven unit controls and the original diagnostic
renderer's seven controls on this same tree. The formatter's final source and
its driver dispatch arms are unchanged; its separately validated 29 focused
controls need not all be rerun merely because the renderer is integrated.
Other platforms compile out the Process/native-run control and therefore run
only five diagnostic CLI tests; that is not a Linux x86_64 integration pass.

These focused tests are not native/backend qualification or a full regression
suite. No CI workflow, aggregate qualification target, or held gate is changed.
