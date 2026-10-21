# Experimental typed projects

The development toolchain can select typed semantics from `oxid.toml`.
Local dependency packages are supported on Linux; native builds and process
execution require Linux x86_64 and LLVM 19.1.7. This workflow is not part of the
published v0.9.0 binaries or a stable v1 edition.

```sh
oxid new hello --edition typed-preview
cd hello
oxid check
oxid run
oxid test
oxid build
./.oxid/bin/hello
```

The scaffold's process entry returns zero on success. Build produces a real
native executable that runs without the source tree or the Oxid compiler.
Install LLVM/Clang/LLD 19.1.7 for building, as described in the
[native preview](../spec/native-preview.md).

```toml
[project]
name = "hello"
version = "0.1.0"
edition = "typed-preview"
entry = "src/main.ox"

[build]
backend = "llvm"
target = "x86_64-unknown-linux-gnu"
profile = "bounded"
entry-mode = "process"

[tests]
smoke = "tests/smoke.ox"
```

`check`, `run`, `build`, and `test` find the nearest project above the working
directory. File commands such as `oxid check src/main.ox` inherit the nearest
manifest above the source. An explicit `--edition` overrides inheritance;
explicit typed file operations retain their existing defaults and flags.
Projects without an edition keep legacy behavior.

Local libraries can be declared with path strings:

```toml
[dependencies]
math = "../math"
```

Every dependency needs its own `oxid.toml` with `edition = "typed-preview"`.
Its entry defaults to `src/lib.ox`; `[project].entry` can select another source
inside that package. Dependencies can declare further local dependencies.
Use `use crate::math::answer;` to import a public item. Within the library,
`crate::` refers to that library's own root. Private declarations and fields
remain private. An importing package can name only its declared dependencies.

The complete graph is checked in one compiler session. Two paths to the same
canonical package directory share record and enum identities; different
directories remain distinct even if their project and type names match.
Aliases must be ASCII identifiers, cannot collide under ASCII case folding,
and cannot exactly shadow a binding in the package's root source file. Cycles
and unsupported dependency forms are errors before execution or build output.

Use source-less `oxid check`, `oxid run`, `oxid build`, and `oxid test` for a
project with dependencies. Inherited file commands with dependencies report
guidance to use these project commands; explicit `--edition` file operations
retain their original standalone contract. Tests retain the declaring project's
dependency graph while replacing only the application entry file.

Create a lock when the current dependency inputs are ready to record:

```sh
oxid lock --offline
oxid check --locked
oxid test --frozen
oxid build --frozen
```

The typed lock records each local package, alias edge, manifest and loaded
dependency source closure. `--locked` requires a matching lock; `--frozen` also
requires offline operation. All dependencies are local in this implementation.
An ordinary project command verifies a lock when present, and does not create
one when absent. The application and test sources remain editable; manifests
and dependency sources require `oxid lock --update` after an intentional change.
Update cannot be combined with locked, offline or frozen mode. Incompatible
legacy, unknown or malformed locks are rejected without overwriting them.

Moving an unchanged relative directory layout preserves the lock. Unloaded
files such as a README are outside its source checksum. The compiler verifies
and executes the same retained dependency source bytes; native artifacts still
depend on the compiler and SDK. See [RFC 0034](../rfcs/0034-typed-local-locking.md)
for the exact lock format and scope.

[Byte count](../apps/byte-count/README.md) is a three-package application outside
the compiler fixtures. It checks shared dependency identity, reads binary stdin,
and builds an executable that can be deployed without its source tree.

Tests are the entry files listed in `[tests]`, sorted by test name. Each runs
as a separate typed process with closed stdin. A test must have a zero-argument
`main() -> i32`: zero passes, nonzero fails. Helpers are loaded through module
declarations, not discovered as additional tests. `test --message-format=json`
reports structured test results without interpreting the program's output as
test results.

`build` always compiles the declared module graph. Rebuilding replaces its
managed executable only after successful compilation and linking. Source or
toolchain errors preserve the previous executable. `compile --output` still
refuses to overwrite an existing output. Avoid simultaneous source edits during
a build; filesystem access uses the compiler's existing stable-tree contract.

`oxid build --incremental` optionally reuses an unchanged LLVM program object.
It still checks the complete source graph, verifies the lock and LLVM IR,
compiles the C runtime and links with the current SDK. Successful output reports
`hit`, `miss` or `unavailable`; JSON adds a `build-cache` event. The key includes
the actual IR, compiler/tool/library bytes and effective environment. Script
tool wrappers and unidentifiable toolchains use ordinary compilation. See
[RFC 0036](../rfcs/0036-native-object-reuse.md) for integrity, storage and limits.

`oxid fmt src/main.ox` prints typed formatting; `oxid fmt --check src/main.ox`
checks it. Directory formatting is not supported. Unsupported project commands
and build settings report an error instead of selecting legacy semantics.

Current limits include bounded execution, call-only borrows, the existing
typed language subset, one native target, and no typed Git/registry dependencies,
vendoring, registry publication, or per-module frontend incrementality. Package graphs are
bounded to 256 packages, 1,024 dependency edges and depth 32, with the existing
256-module limit shared by the complete session. Dependencies rebuild from local
sources each time; the opt-in object cache reuses only native code generation.
See [RFC 0032](../rfcs/0032-typed-project-workflow.md) and
[RFC 0033](../rfcs/0033-typed-path-dependencies.md) and
[RFC 0034](../rfcs/0034-typed-local-locking.md) for the contracts and
[ROADMAP_V2](ROADMAP_V2.md) for the remaining R1 and v1.0 requirements.

## Workspaces

A workspace groups typed packages with one shared input lock:

~~~toml
[workspace]
edition = "typed-preview"
members = ["apps/client", "apps/server", "libs/protocol"]
default-members = ["apps/client"]
~~~

Each member has its own explicit typed-preview manifest. Library members set
kind = "lib" in [project] and default to src/lib.ox; ordinary packages default
to kind = "bin". Imports still require explicit local path dependencies, and
every package keeps its own crate root and type identities.

~~~sh
oxid check --workspace
oxid run --package client
oxid test --workspace
oxid lock
oxid build --workspace --frozen --incremental
oxid doctor --native
~~~

At the root, commands select default-members or all members when defaults are
absent. From a member directory they select that member. --workspace selects all;
--package / -p selects an exact member path or an unambiguous project name.
A root that also declares a package joins as ".", selectable with -p . .

Whole-workspace build checks libraries without creating archives or executables,
and builds each binary in its own .oxid/bin directory. It checks and lowers all
selected binary sources before invoking the first native tool. --incremental
enables each binary's existing object cache; library checks create no cache.
Tests preflight all declared entries and then run separate process children.
Run requires one binary member and preserves its program stdout and status.

The shared version-3 lock lives only at the workspace root. Member-local locks
are conflicts. Existing locks cover the complete roster and dependency union
even when only one member is selected. Imported library source bytes are pinned;
unimported member program and test bodies remain editable. Removing the workspace
declaration while leaving its shared lock cannot silently unlock its members.
Native doctor admits all members and verifies that shared lock without running
or typechecking their source; its report says exactly what was checked.

Workspaces allow 32 members, 256 aggregate retained package nodes, 1024 edges,
1024 loaded files, 8 MiB retained source and 256 selected tests. Ancestor manifests
are admitted as regular files capped at 1 MiB before reading their contents.
Nested/mixed-edition workspaces, globs and escaping member paths are unsupported.
Explicit file editions keep their standalone routes.
See [RFC 0035](../rfcs/0035-typed-workspaces.md) for exact selection, lock,
filesystem and host rules. This workflow remains experimental.

## Source packages

After `oxid lock`, create a source bundle with
`oxid package --frozen --output application.oxpkg.tar`. A workspace bundle always
contains every member and dependency. The command checks retained program and
declared-test sources without executing tests or native tools, and requires a
matching existing lock. Raw manifests, locks and relative sibling dependency
layouts remain unchanged. Unloaded assets and caches are outside the bundle.

Use `oxid package --verify application.oxpkg.tar` to check the bounded canonical
archive. Add `--sha256 HEX` to compare an independently supplied digest.
`oxid package --extract application.oxpkg.tar --output NEW_DIRECTORY` also checks
the isolated source closure and lock before publishing a new destination. It
never replaces an existing directory. Enter the reported project/workspace root
to run ordinary `check/test/build --frozen`; native builds still need Oxid and
the supported LLVM SDK.

See [RFC 0038](../rfcs/0038-typed-source-packages.md) for retained-input semantics,
portable path rules, limits, atomic publication and the JSON contract.
