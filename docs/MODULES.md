# Modules

For relative imports, matching dependency aliases from the project manifest are tried first. Otherwise, the resolver searches the importing source directory, then its `src`, `stdlib`, `modules`, `deps`, and `vendor` subdirectories, followed by the project's `.oxid/deps`, `deps`, and `vendor` directories and entries in `OXID_PATH`, in that order. Absolute import paths are used directly.

Each module is canonicalized, loaded once, and tracked separately while loading so import cycles are rejected. A directory dependency resolves its `oxid.toml` entry or a conventional `src/lib.ox`, `src/main.ox`, `lib.ox`, or `main.ox` entry. Dependency aliases from the root manifest map to local paths or `.oxid/deps/<name>`.

Compilation orders the graph deterministically and strips resolved top-level imports before serialization. Each module retains its own source name and ranges, while source execution records function definition directories for runtime imports. Compiled artifacts do not establish the same directory/relocation contract.

Use `oxid module` to print the module-root categories. `oxid check <entry>` checks the entry file's syntax without resolving its imports; `oxid compile <entry>` resolves and packages top-level imports. Imports inside functions remain runtime operations.


## Initialization order

Source execution runs an import at its top-level position. Artifact compilation places imported module statements before the importer's local statements. For example, an entry file that prints `before`, imports a module that prints `module`, then prints `after` produces:

| Execution path | Output order |
|---|---|
| Source | `before`, `module`, `after` |
| Packaged artifact | `module`, `before`, `after` |

Keep imports before side-effecting entry-file statements when you need the two paths to agree. The [legacy tests](../spec/legacy-0.9.md#modules-and-errors) record this existing difference; they do not claim complete source/artifact equivalence.

## Binding and packaging limits

Import hoisting can change binding availability as well as print order. If an
imported module uses a variable initialized earlier by its importer, source
execution can succeed while the compiled artifact reports an undefined variable.

Only top-level imports are recursively bundled. An import inside a function can
still require its `.ox` file when the artifact runs, so moving the artifact alone
can break that program. Source ranges do not guarantee preserved runtime module
resolution or a self-contained artifact.

The [module contract and corpus](../spec/legacy-modules.md) records matching
cases, missing/cyclic import diagnostics, and these known differences. Future
unified semantics require a separate specification and migration decision.
