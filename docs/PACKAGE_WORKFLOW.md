# Package Workflow Preview

Oxid package projects follow this flow:

1. Create a package with `oxid new` or `oxid init`.
2. Add scripts in `oxid.toml`.
3. Record package metadata and dependencies in the manifest.
4. Use `oxid script <name>` for repeatable workflows.
5. Use `oxid add <name> <target>` to preview dependency edits.
6. Use `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc` during release preparation.

The `packages/demo/` folder shows the expected layout for a user-facing Oxid project.
