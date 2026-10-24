# Source bundles with Git libraries

Status: experimental

Typed Linux x86_64 projects and workspaces can export their locked Git libraries
as ordinary local sources:

```sh
oxid lock
oxid package --frozen --output application.oxpkg.tar
oxid package --verify application.oxpkg.tar
oxid package --extract application.oxpkg.tar --output restored
```

Enter the project root printed by extraction. `oxid check --frozen`,
`oxid run --frozen`, `oxid test --frozen` and `oxid build --frozen` use local
dependencies from the bundle. Git, the original repository and its package
cache are unnecessary. Native rebuilds require LLVM, Clang and LLD 19.1.7.

## Exported files

Creation requires a matching existing typed lock and already cached, pristine
Git checkouts. It never fetches. The source collector retains admitted module,
manifest and declared-test bytes, without including Git metadata or running
project code, tests or build scripts.

Dependency URLs become relative local paths in the exported manifests. The
exported lock records the corresponding manifest digests while preserving the
package graph and source checksums. The original manifests and lock stay
unchanged. Other manifest text, including comments, quoted keys and line endings,
retains its original bytes. Unchanged inputs produce identical archives.

Different commits remain separate packages, even when their public type names
match. Workspace members share a Git cache under the workspace root so a shared
local library resolves the same Git dependency in every member. Separate projects
keep their own caches.

The private materialized archive is admitted again before publication. Extraction
also validates the isolated graph, lock and source inventory. A bundle cannot
request a Git fetch during extraction. Output paths cannot overwrite admitted
inputs or write anywhere inside a Git checkout, including its metadata.

## Limits

The existing source-bundle limits remain: 32 MiB payload, 2,047 payload files,
8 MiB aggregate source bytes, 8 MiB aggregate manifest bytes, 256 packages and
1,024 dependency edges. Each manifest is limited to 1 MiB and a lock to 2 MiB.
Path portability, symlink checks and source visibility rules remain in force.

Public workflows are covered in
[typed_git_dependencies.rs](../tests/typed_git_dependencies.rs).
