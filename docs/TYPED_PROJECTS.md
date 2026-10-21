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
workspaces, vendoring, packaging, or per-module frontend incrementality. Package graphs are
bounded to 256 packages, 1,024 dependency edges and depth 32, with the existing
256-module limit shared by the complete session. Dependencies rebuild from local
sources each time; the opt-in object cache reuses only native code generation.
See [RFC 0032](../rfcs/0032-typed-project-workflow.md) and
[RFC 0033](../rfcs/0033-typed-path-dependencies.md) and
[RFC 0034](../rfcs/0034-typed-local-locking.md) for the contracts and
[ROADMAP_V2](ROADMAP_V2.md) for the remaining R1 and v1.0 requirements.
