# Exact-version private library registries

## Supported workflow

Linux typed projects can publish and consume immutable library releases from an
explicit `file:///` registry. The workflow includes aliases, transitive
dependencies, multiple versions, verified caching, project/workspace locks and
portable source bundles. It remains experimental. Version-range resolution,
HTTPS registries, search, yanking, mirrors, publisher authentication and registry
features are outside this contract.

`oxid registry init NEW_DIRECTORY` creates a registry and refuses an existing
destination. `oxid publish --registry file:///DIRECTORY` publishes a library
from the current project; `--package MEMBER` selects one workspace member.
Publication requires a matching complete author lock, explicit
`edition = "typed-preview"`, `kind = "lib"`, a valid name and one exact version.
The default entry is `src/lib.ox`. Normal library and declared-test entries are
checked before publication. No test body, hook, build script or user program runs.

A consumer declares an exact dependency with three required fields:

```toml
[dependencies]
math = { registry = "file:///srv/oxid", package = "math", version = "1.2.0" }
```

Unknown, duplicate or mixed descriptor fields are refused. Package names use
at most 64 lowercase ASCII letters, digits, `_` or `-`, starting with a letter.
Versions have exactly three canonical unsigned 32-bit decimal components.
Leading zeroes, ranges, prereleases and build metadata are unsupported.
Aliases keep the existing package namespace rules. Quoted string dependencies
retain their existing path or pinned-Git interpretation, including `"1.2.0"`.

Published libraries may depend only on exact releases in the same registry.
Publication rewrites those sources to `registry = "self"`. A consumer binds
`self` to the independently admitted registry that supplied that release.
An application manifest cannot use `self`; a transitive library cannot introduce
another registry, local path or Git authority. Applications may mix existing
local/Git dependencies with explicit registry dependencies.

## Release and cache authority

The registry marker is version 1, kind `typed-private-registry-v1`. A release
directory is `packages/NAME/VERSION/` and contains `release.toml` and `artifact.tar`.
The release record has version 1, kind `typed-library-release-v1`, and binds the
name, package version, typed edition, entry, manifest SHA-256, complete archive
SHA-256 and archive byte count. A complete private staging directory is published
without replacing an existing version.

The artifact uses a separate version 1 inventory kind,
`typed-library-source-v1`, rooted at `source`. Its payload contains the exported
`source/oxid.toml` and the library's own normal/test module closure. It contains
neither transitive package copies nor the author's lock. Payload paths are
strictly ordered, case-safe and collision-free. The existing deployment-bundle
kind and its required root lock remain unchanged; the two readers do not accept
each other's format.

A registry identity is the SHA-256 of four length-prefixed byte strings:

```text
lp("oxid-typed-registry-package-v1")
lp(canonical source URI)
lp(package name)
lp(exact version)
```

`lp` is an unsigned 64-bit little-endian byte length followed by those exact
UTF-8 bytes. File URI spelling is normalized with uppercase percent escapes.
Both the input and canonical URI are bounded to 4096 bytes. Queries, fragments,
credentials, non-normal components, invalid UTF-8 and escaped separators/NUL
are refused. Cache entries live at the lock owner's `.oxid/registry/ID/`.

Aliases of one tuple and dependency diamonds share a package identity. Different
versions or source URIs remain distinct even when their names and archive bytes
are equal. Moving a registry is an explicit source change, not mirror equivalence.

The caller chooses and trusts the registry location on first resolution. SHA-256
checks content integrity; it does not authenticate a publisher. Once a tuple is
recorded in the lock, a different digest or length for that tuple is refused even
with `lock --update`. Upgrading means selecting another exact version. Restoring
the previous manifest and lock selects the previous cached release.

Registry records, markers, artifacts and cached files must be regular files with
one link. Symlinks, hard links, special files, unexpected directories/files and
missing inputs are refused without replacing them. Cached payload bytes are
compared against the verified artifact; source/manifest fingerprints are also
checked against the compiler's retained inputs before execution or native tools.
Cache reuse does not contact the registry and does not trust a marker alone.

## Commands and locks

Ordinary check, run, build, test and lock operations may materialize a missing
exact release. Plain commands do not create a lock automatically. An existing
lock remains authoritative. `--locked`, `--offline` and `--frozen` never fetch,
including from a local file registry. Default documentation, doctor and editor
operations are registry no-fetch too. Existing Git command behavior is preserved.

Graphs with registry packages use lock version 4 and kind
`typed-registry-project-v1` or `typed-registry-workspace-v1`. Existing package,
manifest, loaded-source, alias and membership records remain bound. Each package
adds `registry`, `artifact` and `artifact-bytes`; local packages use empty strings
and zero. Registry IDs use the tuple above; local IDs keep the local-path framing.
The source checksum protocol is unchanged and hashes the actually loaded bytes.
Application and declared-test bodies remain editable.

Graphs without registry packages retain the existing v2/v3 encoding. Switching
a valid graph/lock requires explicit `lock --update`; malformed, unknown and
legacy v1 locks remain non-overwritable. A workspace's existing shared lock covers
every member, even for a selected-member command. Test children repeat admission
and inherit locking modes. Dependency settings never replace the caller's build
or execution policy.

`package --frozen` exports verified cached releases as local dependencies with a
matching local v2/v3 lock. The original manifest and v4 lock stay unchanged. The
complete export is materialized and re-admitted within the private staging
boundary before publication. Extraction therefore supports offline checking and
building without the registry or original cache.

## Resource and failure boundaries

Registry storage uses a lazy operation allowance outside the compiler. Ordinary
manifests and v2/v3 locks without registry dependencies allocate no registry
ledger. Immutable declarations share their allowance through project snapshot
clones; cached origins and workspace members use that same operation context.
Compiler ASTs, source maps, witnesses, default source budgets and native settings
are unchanged.

The metadata ceiling remains 8 MiB. New vectors, strings, paths, reference-counted
allocations, lock pins and traversal queues are admitted before allocation and
their capacities are checked. Existing parser/lock helpers receive conservative
prepayment derived from their bounded inputs and collection growth; retained
capacities stay charged. Actual callback types and complete value/return
transports are separately included. This is a logical request-storage envelope,
not an allocator RSS or operating-system stack limit.

Existing ceilings continue to apply: 256 graph packages, 1024 edges, longest
dependency path 32, and 32 workspace members; 1 MiB per manifest and 2 MiB per
lock; 2047 artifact payload files, 32 MiB payload and 64 MiB archive bytes;
1024 publication source visits and 8 MiB retained source visits. Archive paths
have at most 32 components. Implicit materialization directories are bounded to
4096. Independent limits can reject an operation before another limit is reached.
No registry scope raises the compiler's source or execution ceilings.

Failure before final publication preserves an existing lock, library release or
native output. Cache conflicts preserve the conflicting data for review. Files
created in private staging are cleaned up on failure. Publication never repairs
or overwrites an existing release/cache in order to make validation pass.
A fetch may leave a complete verified cache entry if later in-memory admission
fails. A subsequent offline command revalidates that entry before using it;
the failed command does not remove an already published cache as rollback.

## Acceptance

Acceptance requires real publishing and consumption, aliases and diamonds,
distinct same-byte versions/sources, full workspace authority, explicit upgrade
and rollback, offline cached operation and portable source export. Native checks
must exercise O0/O2 with copied executables after source/cache/registry removal.
Negative controls include malformed descriptors/locks, wrong digests and lengths,
republished tuples, cache mutations and link/special-file boundaries.

Resource controls must cover actual manifest/lock admission, wide and deep cache
trees, exact versus one-less limits, rejection before the corresponding
allocation, preservation of existing cached bytes on reservation failure, and
offline reuse after a late admission failure leaves a complete verified cache.
Existing v2/v3, Git, deployment-bundle and retained compiler assertions remain
mandatory. These checks qualify this registry scope; they do not establish a
complete package ecosystem or v1.0 readiness.
