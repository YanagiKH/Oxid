# RFC 0029: SHA-256 package-tree integrity

Status: proposed; implementation candidate for independent review. Acceptance is
pending, not implied by implementation or local tests. Owner: implementation PR
author. Reviewer: independent PR reviewer. Scope: roadmap M5 integrity increment,
not a complete package-manager, registry, or supply-chain trust milestone.

## Contract and compatibility

The existing Rust package resolver emits SHA-256 for new package lock entries.
`oxid.lock` remains version 1 with algorithm-tagged checksums. A SHA entry is
`sha256:` followed by 64 lowercase hexadecimal digits. FNV entries retain their
historical parser and byte-exact verification behavior, including the distinction
between accepting uppercase hexadecimal syntax and failing an uppercase digest
that differs from the canonical computed lowercase string. Unknown algorithms
and malformed digests fail even in update mode; SHA failure never falls back.

For each existing package name, ordinary resolution hashes with the recorded
algorithm. New names use SHA-256. Mixed locks are supported. Only explicit update
mode upgrades existing FNV entries. `oxid update` (or `oxid lock --update`) keeps
its existing meaning: it can accept content, revision, source and dependency
changes. Review those changes before updating; migration does not authenticate
the newly trusted content. Older binaries explicitly reject SHA checksums.

Resolution options are unchanged. `--locked` requires an existing exact lock and
never writes. `--offline` prevents network-dependent materialization; it is not
a read-only flag and can still create/rewrite a lock for local/cached packages,
including adding/removing dependencies. Neither default nor offline resolution
silently upgrades existing FNV entries. Explicit update remains incompatible
with locked/offline. Existing local Git behavior is preserved.

## Exact byte encoding

Let `u64(n)` encode an unsigned 64-bit integer in little-endian order, and
`lp(b) = u64(length(b)) || b`. SHA-256 hashes this stream:

1. `lp(b"oxid-package-sha256-v1")`.
2. Entries sorted lexicographically by normalized relative UTF-8 path bytes.
3. For each directory: byte `D` then `lp(path)`.
4. For each regular file: byte `F`, `lp(path)`, `u64(file_length)`, then its raw
   bytes (no newline or text conversion).

The root itself has no entry. Empty directories are included. As before, names
`.git`, `.oxid`, `target`, and `oxid.lock` are excluded at every depth before
entry-type validation. Included symlinks, special files and non-UTF-8 components
are rejected. Paths use forward slash separators, are case sensitive, and have
no Unicode normalization. Host metadata, permissions and timestamps are not
hashed. Equality means identical represented trees, not identical executable
permissions or arbitrary Unicode/case equivalents across filesystems.

SHA-only hashing rejects components containing a backslash and duplicate
normalized paths. This prevents a Unix filename `a\\b` aliasing the path `a/b`.
The error directs the user to rename before creating/updating a SHA lock.
Historical FNV framing and filename behavior remain unchanged for compatibility;
its ambiguity is another reason it is not cryptographic integrity.

File data is streamed in 16 KiB chunks; declared versus actual byte length is
checked, rejecting detectable changes. Enumeration retains O(entries + path
bytes) metadata and sorts it. This does not bound tree size/depth, provide a
filesystem snapshot, or defend against malicious concurrent filesystem changes.
Same-length rewrites and metadata races are outside this bounded contract.

## Known-answer vectors

For root file `data.bin` containing bytes `00 ff 41` plus empty directory `empty`,
the complete serialized stream is:

```
16000000000000006f7869642d7061636b6167652d7368613235362d7631460800000000000000646174612e62696e030000000000000000ff41440500000000000000656d707479
```

Its SHA-256 is
`70b7e1ffaa103e8f0e161878a0a53306cf31ea63163c54d67ae141f04aa89c72`,
independently computed using Python `hashlib.sha256(bytes.fromhex(...))`.
A single `value.ox` containing `const value = 1;\n` has legacy FNV digest
`137966a53d847cbc`, independently calculated over the historical framed stream.

## Boundaries, alternatives and acceptance

Use the already pinned `sha2` dependency. Do not alter AST/OXBC checksums, frontend
or provider contracts, PackageId, registry resolution, signatures, publishing,
or trust roots. SHA-256 is content integrity relative to a trusted lock, not
publisher authentication. Retaining FNV indefinitely is compatibility, not a
security recommendation. A mandatory whole-lock v2 migration was rejected for
this slice because the roadmap's complete v2 identity/graph schema is not ready.

Acceptance requires known-answer, tree ordering/mutation/exclusion/binary tests;
legacy preservation and explicit migration; mixed locks; malformed/unknown
algorithms; fail-without-rewrite; Unix filename ambiguity and symlink tests; and
public CLI local dependency tests including current offline semantics. Existing
local Git tests must continue passing. No network access is needed by these
tests. Local verification is scoped to the actual host; hosted CI on the exact
candidate head is required separately. This RFC does not claim v1.0 or M5 done.

Implementation: `src/runtime/packages.rs`. Public CLI acceptance:
`tests/package_integrity.rs`. Run the checks in `CONTRIBUTING.md`; record actual
commands, results and unavailable checks in the validation report.

### Raw filename fixture portability

The observed macOS CI filesystem refuses construction of an invalid UTF-8
filename with EILSEQ (OS error 92), before Oxid reads the directory. The raw-byte
filename rejection fixture is therefore Linux-only; its creation must succeed
and arbitrary I/O failures are not ignored. Included-symlink rejection remains
a separate Unix test. Production filename validation and checksum semantics are
unchanged. macOS qualification requires the updated exact-head hosted tests.
