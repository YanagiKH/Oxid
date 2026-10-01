# Legacy 0.9 module contract

Status: experimental, selected current behavior. Reference revision:
`24b554de13e5cd03f8191cf85ccbdb0c0249ba1b`.

This characterization records the existing interpreter and serialized-AST
compiler separately. It does not select initialization, scope, or import rules
for a new static edition, and does not endorse differences between the two paths.

The corpus is [tests/legacy_modules.rs](../tests/legacy_modules.rs). Run:

```bash
cargo test --test legacy_modules --locked
```

Each test creates an isolated project containing real `.ox` files and invokes the
public CLI. [Validation results](../docs/architecture/module-validation.md) record
the tested platform and remaining limits. The fixture source is embedded in the Rust test so the existing
recursive `oxid test` command does not accidentally run diagnostic-only inputs
as successful application tests. There are no network or package downloads in
this corpus, and `OXID_PATH` is removed from each child environment.

## Covered cases

| Case | Source execution | Compile / artifact execution |
| --- | --- | --- |
| Same module through `module.ox` and `./module.ox` | Module initializes once | Same result |
| Diamond imports with a shared leaf | Leaf, left, right, entry initialize in that order; leaf runs once | Same result when imports precede local statements |
| Nested top-level relative imports, unrelated working directory | Resolve from the importing source; ignore the wrong working-directory fixture | Same result |
| Missing top-level module | Exit 1; responsible import location and missing filename | Compilation exits 1 with missing filename; no artifact written |
| Cyclic top-level imports | Exit 1 with cycle reason and repeated module name | Compilation exits 1; no artifact written |
| Error in imported module | Division-by-zero reason and the imported module's line 2 | Compilation succeeds; execution reports the imported module's line 2 |
| Import uses a binding declared earlier in its importer | Binding is available | Dependency executes first; runtime reports an undefined identifier |
| Import inside entry `main` | Dependency is loaded when the function runs | It remains a runtime source dependency; moving only the artifact causes a missing-module error |

The test names identify each row. Diagnostics assert exit status, a meaningful
reason, filenames, and available source ranges; they do not fix host-specific
OS wording or temporary absolute paths as portable contracts.

## Identity and resolution

The covered aliases resolve to the same canonical file. A successful load is
remembered, and a module currently loading participates in cycle detection.
Top-level source-relative resolution also works when resolving the same import
from the CLI's working directory would select a different file.

This does not specify all package aliases, `OXID_PATH` precedence, symlink/hardlink
identity, case-insensitive filesystems, directory entry conventions, or module
visibility/name collision behavior. Those require separate corpus cases and
accepted rules.

## Initialization and binding availability differ

The source interpreter executes an import where it is encountered and imported
statements use the global environment. The current artifact compiler visits
top-level dependencies before appending the importer's non-import statements.
This changes both side-effect order and available bindings.

For an entry containing `let answer = 42;` followed by an import whose module
prints `answer`, source execution prints `42`. The compiled artifact fails at
that module statement because `answer` has not yet been initialized. This is a
known limitation, not compile-time name resolution or static checking.

Keep top-level imports before local initialization when relying on the
characterized matching paths. That guideline does not prove equivalence for
all programs; the earlier [legacy characterization](legacy-0.9.md#modules-and-errors)
also records a visible print-order difference.

## Function-local imports are not bundled

The compiler recursively resolves only imports appearing as top-level statements
of each visited module. An import inside a function remains an AST operation for
the runtime. A generated `.oxb` therefore need not contain every source dependency.

The covered entry function imports `module.ox` and prints its `answer` binding.
Both paths work while the module is beside the artifact; copying only the
artifact to a fresh directory makes its runtime import fail. The test leaves the
original source files intact and does not depend on removing them.

This case does not promise relocation support or preservation of every function's
original source directory in compiled artifacts. Source maps identify diagnostic
locations; they are not a complete module environment or a native linking model.

## Future semantic decision

Before unifying these paths or introducing a new edition, an RFC must choose:

- whether imports are declarations or executable statements
- module initialization order and when bindings become available
- namespace/visibility rules and handling of import cycles
- whether function-local imports are supported, rejected, or explicitly dynamic
- artifact relocation and completeness guarantees
- diagnostics, compatibility, and migration for existing legacy programs

Any accepted behavior change needs separate tests for the new contract while
keeping explicit coverage of the supported legacy route. This corpus alone does
not complete M0 or establish a full module system.
