# Compiler

Oxid compilation is described as a visible staged workflow.

## Helpers

- `compile_plan(project_name, version, entry_point)`
- `compile_snapshot(project_name, version, entry_point)`
- `compile_phases()`
- `compile_command_list()`
- `compile_hint()`

## Output shape

The compiler preview is intentionally compact:

1. a package-based banner
2. the current compilation phase list
3. the script surface used during bootstrap and self-hosting
4. a short note describing the Rust fallback boundary

## Intended usage

- `tools/compile.ox` prints the compiler snapshot
- `examples/compile_preview.ox` shows the minimal preview path
- `tests/compiler_smoke.ox` exercises the compiler helper path
