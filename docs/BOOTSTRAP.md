# Bootstrap

Oxid bootstrap should keep the native edge as small as possible while still describing the full build and toolchain workflow.

## Goals
- keep Rust isolated to the fallback boundary
- move project orchestration into Oxid modules
- keep bootstrap output readable
- include compile and self-compile snapshots in the bootstrap path

## Helpers
- `bootstrap_plan(project_name, version, entry_point)`
- `bootstrap_snapshot(project_name, version, entry_point)`
- `bootstrap_summary(project_name, version, entry_point)`
- `bootstrap_manifest(project_name, version, entry_point)`
- `bootstrap_command_list()`
- `bootstrap_boundary_note()`

## Entry point

The repository now exposes bootstrap through the Oxid tool scripts in `oxid.toml` so users can run the workflow directly from the source tree:

```bash
oxid script bootstrap
```

The same preview is also available as `tools/bootstrap.ox` for direct execution through the interpreter.