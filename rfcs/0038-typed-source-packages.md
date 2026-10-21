# RFC 0038: retained typed source packages

Status: experimental implementation extending RFCs 0033–0035. Creation and
extraction retain the existing Linux source-loading and Linux x86_64 process
preflight qualification. Format verification itself executes no compiler code.

## Commands and source scope

```sh
oxid lock
oxid package --frozen --output application.oxpkg.tar
oxid package --verify application.oxpkg.tar --sha256 EXPECTED_SHA256
oxid package --extract application.oxpkg.tar --output new-directory
# Enter the reported root under new-directory, then:
oxid check --frozen
oxid test --frozen
oxid build --frozen
```

`package` creates a source bundle for a standalone typed project or the entire
enclosing workspace. It includes all workspace members regardless of defaults
or the current member. `--workspace` explicitly requires workspace membership;
package selectors are unsupported. An existing matching typed v2/v3 lock is
always required. `--locked`, `--offline` and `--frozen` are accepted without
fetching, creating or updating a lock. Creation does not invoke network access,
native tools, scripts or test processes. Target/profile/entry-mode overrides,
incremental flags and lock updates are rejected.

The default output is `<lock-owner>/.oxid/package/source.oxpkg.tar`. An explicit
`--output` selects a different archive path. Verify and extract own their input
before project discovery, so an unrelated or broken current project cannot
select their source inputs. Verify accepts no output. Extract requires a new
destination with an existing ordinary parent. All suboperations accept
`--edition=typed-preview` and `--message-format=text|json`.

Creation checks the normal package/library sessions and every declared test in
every included package, including dependency-package tests, without executing
them. Tests must satisfy the existing process entry rules even though their
bodies are not run. Dependencies use the already admitted graph rerooted for
the declaring test package. Invalid source, entry signatures or locks prevent
publication. This command does not introduce new language or process semantics.

The bundle contains exactly the raw admitted manifests, authoritative retained
lock bytes and actual loaded source/test closures. No directory scan supplies
extra payload. A loaded source below `.oxid` or `target` is included; unrelated
caches, Git metadata, credentials, outputs and assets are absent. Asset include
patterns, compression, registry resolution, signing and compiler/runtime
bundling are separate work.

## Retention and relocation

The package collector retains raw manifests in shared immutable buffers from
the same bounded read that parsed and hashed them. Ordinary project commands
keep parsed metadata and digests, releasing the temporary raw manifest buffer;
the source-package retention budget applies only to package collection. The lock's admitted bytes are retained without
a later filesystem reread. A borrowed visitor exposes the compiler session's
actual source buffers. Different sessions that name the same canonical input
must agree byte-for-byte. This prevents packaging later live bytes under an
earlier checked identity.

The common ancestor of all admitted package roots and the lock owner anchors
the layout. Every payload path starts with `source/`; the inventory records the
lock owner's relative root, such as `source/application`. Local sibling
dependencies keep their original relative layout, raw manifests and unchanged
lock identities. Equal package names or equal source bytes never merge distinct
package directories.

Absolute dependencies, symlink components and non-normalized spellings are
rejected before reading their targets. Dependencies may have leading `../`
components followed by ordinary components. Empty components, `./` and an inner
`a/../b` are unsupported because the source bundle does not invent traversal
directories absent from its retained roster.

Before publishing, creation materializes its retained plan in fresh private
storage and re-admits that complete project with an explicit boundary. Discovery
stops at this boundary, and every package/source resolution must remain inside
it. The resulting root and sorted file bytes must equal the original plan.
Extraction performs the same admission and comparison before exposing any
destination. It cannot borrow a missing dependency from the original tree or
an enclosing workspace. Verify checks the archive format and integrity only;
it does not perform project admission or establish language validity.

## Canonical archive

The format is an uncompressed POSIX ustar with `OXID-PACKAGE.toml` first,
followed by regular payload files in UTF-8 path-byte order. Headers have file
mode 0644, uid/gid/mtime zero, empty owner names, and canonical numeric fields.
No directory entries, links, devices, sparse files, GNU/PAX extensions or
concatenated archives are accepted. Padding is zero; exactly two zero blocks
and EOF terminate the archive. Paths must fit ustar's name/prefix fields without
truncation. Ordinary tar tools can read the output; the Oxid reader deliberately
accepts only this canonical source-package format.

The inventory is a strict, canonical TOML subset:

```toml
version = 1
kind = "typed-source-package-v1"
root = "source/application"

[[files]]
path = "source/application/oxid.lock"
size = 123
sha256 = "<64 lowercase hexadecimal digits>"
```

Every payload file has one sorted record. Unknown/duplicate fields, noncanonical
numbers, duplicate paths, case aliases, file/parent conflicts, mismatched
lengths/hashes, bad headers, missing/extra members, truncation and trailing
bytes are errors. Root `oxid.toml` and `oxid.lock` must be present. Relative
UTF-8 paths reject traversal, empty components, backslashes, controls, reserved
device names and platform-special characters. These restrictions are not a
claim of support for every Unicode case-folding filesystem.

Archive bytes contain no machine paths, wall clock times, staging names or
compiler binary identities. Repeated packaging and relocation of the entire
unchanged layout produce identical archive bytes. This determinism applies to
the source bundle, not generated native executables.

Embedded file hashes detect corruption. Optional `--sha256 HEX` on verify or
extract compares the entire archive against a separately supplied digest;
neither an embedded hash nor an unauthenticated digest authenticates a publisher.

## Bounds and filesystem effects

Admission uses existing graph and session limits, with operation-wide budgets:
256 packages, 1024 edges, 256 declared tests, 1024 retained source files and
8 MiB retained source bytes. Repeated sessions count their retained inputs.
Raw manifests allow 1 MiB each and 8 MiB in total, checked during loading;
locks allow 2 MiB. The inventory allows 1 MiB, payload 32 MiB, archive 64 MiB,
and at most 2047 payload files plus the inventory. Length/count arithmetic is
checked before allocation or insertion. Materialization additionally allows at
most 32 path components and 4096 implicit directories, checked before creating
payload directories. A format-valid archive outside these materialization
limits can verify successfully but cannot be extracted.

Publication writes, syncs and reads back a complete archive in a fresh private
sibling directory, then uses one same-filesystem rename. Only an absent or
non-symlink regular output may be replaced. Input-path and hard-link collisions,
special files and directory outputs are rejected. Failures before publication
preserve previous archive bytes. The SHA is returned in the summary; there is
no second checksum file requiring a multi-file transaction.

Extraction fully verifies the bounded archive before staging. It exclusively
creates regular files, sets payload mode 0644 and nested directories 0755,
and retains a private 0700 destination root. It validates staged sources and
locks, syncs files/directories and publishes using Linux
`renameat2(RENAME_NOREPLACE)`. Any existing destination, including an empty
directory or dangling symlink, is preserved; concurrent extractors cannot
replace each other. Missing atomic publication support fails without a fallback
overwrite. Owned temporary storage is cleaned on failure. A post-publication
directory sync error reports that publication already occurred.

The stable-parent filesystem contract matches existing managed outputs. This
is not a sandbox against hostile concurrent ancestor replacement. Ordinary
archive reads reject nonregular files before opening and use no-follow,
nonblocking opens plus descriptor/path identity checks on Unix.

## Reporting and acceptance

JSON emits one `package-summary` record with schema
`oxid-typed-package-summary-v1`, schema_version 1, edition `typed-preview`,
operation `create|verify|extract` and success. A success includes absolute archive
path, whole-file sha256, bytes, payload files, relative root, and output (archive
path for create, null for verify, destination directory for extract). A failure
includes error. Exit status is 0 on success, 1 on command failure and 74 when
reporting fails. Human extraction output identifies the directory to enter.

Format unit tests cover hostile headers, inventory bounds, path conflicts,
short I/O and exact EOF. Public CLI tests cover admitted closures, declared test
validation without execution, lock drift, full workspaces, relocation and
atomic failure preservation. An independent Python tarfile/TOML/SHA-256 oracle
checks every header/index/member, repeated and relocated determinism, both Rust
compiler profiles and real byte-count projects rebuilt offline after deleting
their complete original source trees. CI retains these results separately from
the historical certificates and retained single-package Unit2 assertions.

This increment supplies portable local source bundles. It does not complete
the remaining R1 package manager or v1 language, runtime and release gates.
