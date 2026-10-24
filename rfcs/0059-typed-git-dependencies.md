# Pinned Git dependencies

Status: experimental

Typed projects on Linux can use libraries from Git repositories. Install Git
before resolving or checking these dependencies:

```toml
[dependencies]
codec = "git+https://github.com/example/codec.git#0123456789012345678901234567890123456789"
```

Replace the example repository and commit with your library's actual values.
Use a complete 40- or 64-character commit hash. Branches, tags, embedded
credentials and URL query strings are rejected. HTTPS is required for remote
repositories; `git+file:///absolute/path#COMMIT` also supports local repositories.
The library must have a typed `oxid.toml` and an entry such as `src/lib.ox`.
Import its public API through the declared alias, for example
`use crate::codec::decode;`.

```sh
oxid lock
oxid check --frozen
oxid test --frozen
oxid build --frozen --profile release
```

## Versions and locks

Each application stores Git checkouts under `.oxid/deps/`. The cache identity
includes the repository URL and commit. Two aliases for the same repository and
commit share one package and its nominal types. Different commits can coexist
even when their manifests have the same package name and version; their types
remain distinct. Transitive Git dependencies use the same identity rules.

The typed lock records the resolved package graph, manifest digests and source
checksums. Existing locks are verified before execution. Changing a pin requires
`oxid lock --update`; ordinary commands preserve an existing lock. Workspace locks
continue to cover every member, including members outside the current selection.

`--locked`, `--offline` and `--frozen` require cached Git commits and never fetch,
including for file URLs. `--locked` and `--frozen` also require a matching lock.
`oxid doctor --native` checks cached packages without fetching them. Once cached,
an unchanged project can check, run, test and build while its Git remote is
unavailable. Built executables require neither Git nor the package cache.

## Cache contents and local packages

The existing Git cache protections also apply here. Edited, staged, untracked or
ignored files cause refusal before that checkout's configuration, index or HEAD
is changed. Symlinks and unsupported Git metadata layouts are refused. Oxid does
not discard local files. Preserve your changes elsewhere or move the affected
cache directory aside before fetching again. Earlier dependencies may already
have resolved when a later one fails.

A Git package may use relative local dependencies within its checkout. Such an
edge cannot escape the checkout, including through a symlink or another alias
of the same cached package. A separately pinned
Git dependency remains available for libraries in another repository.

Source bundles currently require portable local dependency paths. A project
using Git dependencies cannot yet be exported with `oxid package`; the command
reports the unsupported source before publishing an archive. Registry versions,
automatic version selection, signatures and source vendoring remain unavailable.

Public workflow checks are in
[typed_git_dependencies.rs](../tests/typed_git_dependencies.rs).
