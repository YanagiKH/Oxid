# Example Oxid package

A small project to run, edit, and use as a starting point. From the repository root:

```bash
cd packages/demo
oxid run src/main.ox
oxid test
oxid build
```

The generated artifact is `.oxid/bin/demo.oxb`; run it with `oxid run .oxid/bin/demo.oxb`.

## Files

- `src/main.ox`: application entry point
- `src/lib.ox`: reusable helpers
- `tests/smoke.ox`: smoke test
- `oxid.toml`: package metadata and scripts

Use `oxid script run`, `oxid script test`, or `oxid script doctor` for the commands declared in the manifest. See the [project workflow](../../docs/PACKAGE_WORKFLOW.md) for dependencies and packaging.
