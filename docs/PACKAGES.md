# Packages and dependency locking

`oxid.toml` supports `project.name`, `project.version`, `project.entry`, `[scripts]`, `[dependencies]`, `[build]`, and `[features]`.

## Dependency sources

Path dependencies may be absolute or relative to the manifest that declares them:

```toml
[dependencies]
shared = "../shared"
```

Remote dependencies use HTTPS Git and must pin a complete 40- or 64-character commit hash. Replace this illustrative URL and hash with a real repository and commit:

```toml
[dependencies]
codec = "https://github.com/example/oxid-codec.git#0123456789012345678901234567890123456789"
```

`git+https://` is also accepted. `git+file://` is available for absolute local Git repositories and testing. Branch names, tags, credentials in URLs, query strings, unpinned revisions, and non-HTTPS remote schemes are rejected.

## `oxid.lock`

Resolution writes deterministic `oxid.lock` version 1. Entries are sorted and contain the package name, normalized source, pinned Git revision when applicable, package-tree checksum, and sorted nested dependencies. Git checkouts live in `.oxid/deps/<name>`; modules can import a dependency by its manifest alias.

Nested manifests are resolved recursively. The resolver rejects cycles, conflicting sources for one name, unsafe names, checksum changes without an explicit update, and lockfile fields or versions it does not understand.

## Commands

```bash
oxid add shared ../shared
oxid remove shared
oxid list
oxid lock
oxid fetch
oxid update
oxid install
oxid build --locked
oxid build --offline
oxid build --frozen
```

- `--locked` requires an existing matching lockfile and does not access the network.
- `--offline` allows only local paths and already cached Git commits.
- `--frozen` combines locked and offline behavior.
- `update` allows an intentional revision or checksum transition and rewrites the lockfile.
- `install` resolves dependencies and builds the project.

Commit `oxid.lock` for applications. Libraries may commit it when reproducible repository tooling is important, while consumers still resolve from their own manifest.


## Preserving Git cache contents

Existing Git caches must be pristine before resolution can change their Git
configuration, fetch objects or check out a revision. Modified, staged, deleted,
untracked **and ignored** files cause refusal in ordinary, locked, offline,
frozen and update modes. Ignored build products are deliberately rejected even
when they would not conflict with the current revision: an uncached revision
could track those paths, and discovering its contents would require fetching
before preservation is established. This is stricter than earlier releases,
which could accept ignored files or overwrite tracked edits during checkout.

Git index entries marked `assume-unchanged` or `skip-worktree` are also refused,
even when their files are unchanged, because ordinary status checks can conceal
local edits under these flags. Unsupported submodules are refused before mutation,
including Git index links without `.gitmodules` metadata; their nested work trees
are not admitted by an outer status check. Oxid does not clear flags or delete local files.
Preserve your changes outside the cache first, then manually restore tracked
files and move untracked/ignored products out of the cache. If you set index
flags, clear them manually after preserving your work. Alternatively, move the
whole cache aside and let Oxid create a fresh cache when fetching is allowed.

For these preflight refusals, the affected cache's files, HEAD, index and Git
configuration and the project lockfile remain unchanged, and no fetch is
attempted for that cache. This is a per-cache check, not a rollback of earlier
successfully resolved dependencies. Fresh cache creation and pristine-cache
revision updates remain supported. Checkout never uses force and also refuses
to overwrite ignored files. These guarantees assume a stable filesystem; they
do not claim protection against concurrent writers or hostile Git configuration.

## Current limits

The resolver identifies packages by name and cannot resolve multiple versions of the same name together. New package-tree checksums use SHA-256. Existing FNV entries retain their historical verification algorithm until an explicit update; mixed-algorithm locks are supported. Neither digest authenticates a publisher or establishes source trust. Review dependency sources and lockfile updates before running their code. Registry publishing is not part of the 0.9 workflow.

## Checksum migration

`oxid update` upgrades existing FNV entries to SHA-256 while retaining its usual
permission to accept source, revision and content changes. Review dependencies
before updating. Ordinary resolution preserves recorded algorithms; `--locked`
verifies without rewriting. `--offline` retains its existing ability to create or
rewrite locks from local/cached content, but does not upgrade existing FNV entries.
Older Oxid binaries reject SHA-256 checksums explicitly.

SHA hashing uses deterministic, domain-separated binary framing. Included
symlinks, special files, non-UTF-8 names and backslash-containing components are
rejected; rename ambiguous components before migration. The exclusions `.git`,
`.oxid`, `target`, and `oxid.lock` apply at every depth. File bytes and empty
directories are included; permissions and timestamps are not. A concurrently
changing tree is not an atomic snapshot. See [RFC 0029](../rfcs/0029-package-sha256-integrity.md)
for the exact representation and bounded trust contract.
