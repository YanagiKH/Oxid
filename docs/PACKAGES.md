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


## Current limits

The resolver identifies packages by name and cannot resolve multiple versions of the same name together. Package-tree checksums use FNV; they detect changes but are not cryptographic supply-chain verification. Review dependency sources and lockfile updates before running their code. Registry publishing is not part of the 0.9 workflow.
