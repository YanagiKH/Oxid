# RFC 0034: typed local dependency input locks

Status: experimental implementation; extends [RFC 0033](0033-typed-path-dependencies.md).

## User outcome

A project can record its admitted local dependency graph in `oxid.lock` and
reject unexpected input changes before running programs or publishing a native
build. The application and its tests remain editable. This contract pins
dependency inputs; it does not promise identical machine code across SDKs,
compiler versions, checkout paths or operating systems.

```sh
oxid lock --offline
oxid check --locked
oxid test --frozen
oxid build --frozen
# After reviewing intentional dependency or manifest changes:
oxid lock --update
```

## Command contract

| Command or option | Behavior |
|---|---|
| `lock` | Create a missing typed lock; verify a matching existing lock. Changed inputs require explicit update. |
| `lock --update` | Create or replace a valid typed lock from the currently loaded graph. Reject legacy, unknown or malformed lock formats without overwriting them. |
| `--locked` | Require an existing, matching lock for `check`, `run`, `build`, `test` or `lock`. |
| `--offline` | Forbid fetching. All dependencies in this increment are local; this option does not select the legacy resolver or add network support. |
| `--frozen` | Combine `--locked` and `--offline`. |
| Ordinary project commands | Work without a lock and do not create one. If a lock exists, verify it. |

`--update` is valid only with `lock` and conflicts with `--locked`, `--offline`
and `--frozen`. An ordinary `--offline` command does not create a lock. These
options belong to source-less typed project operations; explicit file and
legacy commands preserve their existing contracts.

Each declared test child receives the applicable lock options, reloads the
manifest graph and verifies its retained dependency sources. The selected test
replaces only the application source entry used for execution; it does not
change the normal application entry recorded in the lock. Lock verification
precedes user effects and the native toolchain. Failure preserves both the lock
and the previous managed executable.

## Locked inputs and identities

Canonical package directories identify nodes as in RFC 0033. The lock represents
their locations relative to the canonical application directory: `.` for the
application and relative paths, including parent components when needed, for
dependencies. Moving an unchanged relative directory layout preserves identity.
Absolute manifest dependency paths still designate their original absolute
locations; the lock does not rewrite manifests during relocation.

Each package records its identity, location, name and version metadata, edition,
normal entry path and SHA-256 of the raw manifest bytes. Version strings do not
enable semantic-version resolution. Aliases are directed edges between package
IDs. Distinct roots may have identical names and versions; repeated aliases and
diamonds share one row for a canonical root.

Each dependency additionally records its complete loaded source closure: its
entry and every recursively declared `mod` file, including unused declarations.
Paths and raw bytes contribute to the checksum. Directory names such as
`target` or `.oxid` have no exclusion meaning for this closure. Files that are
not loaded, such as an unrelated README, do not contribute.

The application's source checksum is the marker `root`. Its source and declared
test bodies can change without updating the lock. Its raw manifest and graph
remain pinned. A lock also does not pin the Rust compiler, LLVM/SDK, environment,
runtime inputs or native output bytes.

## File schema and checksum framing

The file is a strict TOML subset capped at 2 MiB, separate from legacy lock
version 1. It uses the generated field/table order and accepts only the string
escapes `\\` and `\"`; comments and reordered fields are outside this schema:

```toml
version = 2
kind = "typed-local-v1"
root = "<application package ID>"

[[package]]
id = "<package ID>"
path = "."
name = "example"
version = "0.1.0"
edition = "typed-preview"
entry = "src/main.ox"
manifest = "<SHA-256 of raw oxid.toml bytes>"
sources = "root"

[[dependency]]
owner = "<declaring package ID>"
alias = "math"
package = "<target package ID>"
```

Package records are sorted by ID and dependency records by
`(owner, alias, package)`. Missing, duplicated or unknown fields, invalid values
and unsupported formats are rejected. Writes use a temporary regular file and
atomic publication; readers reject symlinks and special files. Existing legacy
lockfiles require an explicit migration outside this format's commands.

Let `lp(bytes)` be an unsigned 64-bit little-endian byte length followed by
those bytes. All paths below are the exact UTF-8 paths defined by the loader and
relative-location contract; concatenation is byte concatenation.

* Package ID is lowercase hexadecimal SHA-256 of
  `lp("oxid-typed-local-package-v1") || lp(relative canonical location)`.
* Manifest digest is lowercase hexadecimal SHA-256 of the raw manifest bytes.
* Dependency `sources` is `sha256:` followed by lowercase hexadecimal SHA-256 of
  `lp("oxid-typed-local-source-v1") || lp(relative entry path) || u64le(file count)`
  followed by path-byte-sorted records
  `"F" || lp(relative source path) || lp(raw source bytes)`.

Length framing binds boundaries, paths and contents without relying on filenames
or delimiters that cannot appear in input. The compiler calculates source
digests from its retained source buffers, then checks or executes that same
prepared session. It never verifies one read and executes a later source read.
The existing stable-tree filesystem admission rules and package resource bounds
still apply. Lock creation parses the source graph to discover its closure;
it is not a substitute for successful type checking or tests.

## Verification and remaining scope

Compiler tests retain a prepared session, change files on disk and verify that
execution still uses the prepared bytes. CLI tests cover graph identity,
relocation, same-name packages, corrupt/incompatible locks, input changes,
declared tests, bounded readers and failed-write preservation.

`scripts/verify_typed_package_lock.py` checks the three-package byte-count
application with Python's independent TOML parser and SHA-256 implementation.
It exercises locked tests and stream execution, relocation, editable application
sources, dependency rejection and explicit update, loaded modules beneath
`target`/`.oxid`, and optional native build/preservation checks. The independent
byte-count oracle verifies results before and after an accepted dependency edit.

Registry/Git resolution, workspaces, semantic-version selection, vendoring,
packaging, incremental caches and native artifact reproducibility remain
separate work. This increment does not complete R1 or a v1 release gate.
