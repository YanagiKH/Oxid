# R1 explicit workspace selection

This ordinary functional slice builds on `fe5def78e334886bc222192235160a48f2c0077e`.
It does not claim complete R1 support or source qualification.

A virtual manifest contains exactly one workspace table and one members key:

```toml
[workspace]
members = ["app", "library"]
```

The array parser consumes quoted strings, separators, comments and line endings
explicitly. It shares the existing manifest string escapes, including `\uNNNN`;
commas inside strings are path characters. Multiline arrays and a trailing comma
are accepted. Empty arrays, duplicate keys/tables, other workspace settings,
mixed project/dependencies tables, globs and nested workspaces are rejected.

Each member is an explicit relative directory containing `oxid.toml`. Paths are
resolved from the virtual manifest's lexical parent. Explicit sibling paths
such as `../library` are supported; this is not a member containment sandbox.
Canonical manifest identity detects duplicate members, including symlink aliases,
and membership cycles. No directories are searched for candidate projects.

Every member manifest must declare typed-preview edition, an explicit valid entry
and a unique project name matching `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`. All listed
manifests are parsed and validated in declared order before selection. An invalid
unselected manifest therefore fails selection. Unselected sources are not opened
unless that member is reached through the selected package's explicit dependencies.
Dependency declarations in all member manifests are validated; dependency graph
traversal and cycle checks cover the selected member's closure.

```sh
oxid check --manifest-path workspace/oxid.toml --package workspace-app
oxid run --manifest-path workspace/oxid.toml --package workspace-app
oxid build --manifest-path workspace/oxid.toml --package workspace-app
oxid test --manifest-path workspace/oxid.toml --package workspace-app
```

`--package` is a command-local option and selects a declared project name, never
a path. Its split and `=` forms are accepted once. With one member it is optional;
with multiple members omission fails before source loading and lists names in
declared order. Unknown names fail with the same list. A direct project manifest
rejects `--package`; its behavior without the selector is unchanged. Global error
priorities, sticky JSON, script arguments and `--` operand ownership remain in force.
Native RUN and all TEST JSON requests are rejected before manifest I/O as before.

The selected member enters the existing immutable compiler session as application
package zero. Its entry, edition and lexical output parent drive every command;
main and test discovery use its original root. Membership grants no imports.
Dependencies remain package-local, canonical identities with explicit direct aliases.
Workspace validation retains the original bytes and parsed member snapshot, so
graph resolution reuses a declared dependency without reading its manifest again.
The same stable-filesystem assumption as direct manifest routes applies; this is
not an atomic snapshot of concurrent filesystem edits.

The virtual root, every member and newly reached dependencies share the existing
16,384-byte aggregate manifest input allowance. Reusing a canonical snapshot does
not charge input twice. Member/snapshot rows, decoded member paths and dependency
edges are admitted before vector/string growth within the existing edge envelope.
Retained paths consume the existing project path allowance. Source, token, node,
module and index limits are shared by the selected graph, never multiplied by
workspace membership. No limit constants are increased.

Native BUILD/RUN and native TEST keep existing publication and execution policies.
Artifacts belong to the selected member; the virtual root receives no application
artifact. TEST retains its separate namespace and prepares every case before
running one. Real native execution requires the separately authorized
LLVM 19.1.7/Linux x86_64 toolchain; ordinary tests do not establish native acceptance.

No workspace fan-out, default-members setting, implicit discovery, network fetch,
lockfile write, cache, publishing or new target/profile/engine is included.
The external example is `examples/projects/typed_workspace_selection/`.
Focused CLI controls are in `tests/workspace_selection_core.rs`; its ignored native
control is reserved for the authorized toolchain environment.
