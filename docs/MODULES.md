# Modules

Imports are resolved relative to the importing source first, then through project roots including `source`, `src`, `stdlib`, `modules`, `.oxid/deps`, `deps`, `vendor`, and `OXID_PATH`.

Each module is canonicalized, loaded once, and tracked separately while loading so import cycles are rejected. A directory dependency resolves its `oxid.toml` entry or a conventional `src/lib.ox`, `src/main.ox`, `lib.ox`, or `main.ox` entry. Dependency aliases from the root manifest map to local paths or `.oxid/deps/<name>`.

Compilation orders the graph deterministically and strips resolved top-level imports before serialization. Each module retains its own source name and ranges, and functions retain the directory where they were defined so nested relative imports do not accidentally resolve from the caller.

Use `oxid module` to print the module-root categories. `oxid check <entry>` checks the entry file's syntax without resolving its imports; `oxid compile <entry>` resolves and packages the import graph.


## Initialization order

Source execution runs an import at its top-level position. Artifact compilation places imported module statements before the importer's local statements. For example, an entry file that prints `before`, imports a module that prints `module`, then prints `after` produces:

| Execution path | Output order |
|---|---|
| Source | `before`, `module`, `after` |
| Packaged artifact | `module`, `before`, `after` |

Keep imports before side-effecting entry-file statements when you need the two paths to agree. The [legacy tests](../spec/legacy-0.9.md#modules-and-errors) record this existing difference; they do not claim complete source/artifact equivalence.
