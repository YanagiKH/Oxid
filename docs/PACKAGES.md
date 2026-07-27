# Packages

`oxid.toml` now supports:

- `project.name`
- `project.version`
- `project.entry`
- `[scripts]`
- `[dependencies]`
- `[features]`
- `oxid add <name> <target>` for quick manifest editing
- `oxid script <name>` for script-first workflows

This keeps the project manifest usable for future dependency resolution and publishing.
