# Package Workflow Preview

Oxid package projects are intended to feel like a normal project workspace with a lightweight manifest and reusable source modules.

## Suggested flow

1. Create a package with `oxid new` or `oxid init`.
2. Put the application entry in `src/main.ox`.
3. Put reusable helpers in `src/lib.ox`.
4. Put Oxid modules shared across projects in `stdlib/`.
5. Add scripts in `oxid.toml`.
6. Preview dependency edits with `oxid add <name> <target>`.
7. Use `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc` during release preparation.

## Why this structure works

- `src/main.ox` gives you a stable app entry.
- `src/lib.ox` gives you a reusable local module.
- `stdlib/` gives you shared language-level helpers.
- `tests/` gives you smoke coverage for package behavior.
- `examples/` gives you runnable demonstrations for users.

## Demo package

The `packages/demo/` folder is the reference layout for a user-facing Oxid project.
