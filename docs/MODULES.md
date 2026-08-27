# Modules

Imports are resolved relative to the importing source first, then through project roots including `source`, `src`, `stdlib`, `modules`, `.oxid/deps`, `deps`, `vendor`, and `OXID_PATH`.

Each module is canonicalized, loaded once, and tracked separately while loading so import cycles are rejected. A directory dependency resolves its `oxid.toml` entry or a conventional `src/lib.ox`, `src/main.ox`, `lib.ox`, or `main.ox` entry. Dependency aliases from the root manifest map to local paths or `.oxid/deps/<name>`.

Compilation orders the graph deterministically and strips resolved top-level imports before serialization. Each module retains its own source name and ranges, and functions retain the directory where they were defined so nested relative imports do not accidentally resolve from the caller.

Use `oxid module` to print the active module-root categories and `oxid check <entry>` to validate a graph without running it.

