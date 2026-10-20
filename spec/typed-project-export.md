# Normalized typed compiler-source export

```
oxid export --manifest-path PATH --output NEW_DIRECTORY
oxid export --manifest-path WORKSPACE --package NAME --output NEW_DIRECTORY
```

This explicit command checks the selected typed-preview package and all of its
transitive local dependencies, then exports a normalized, self-contained
compiler-source tree. It does not execute source, invoke native tools, build an
executable, download packages or modify the original project. A library-like
selected package does not need a main function. All dependency bodies are
checked, including unused bodies, through the ordinary immutable compiler
session and checker; no checks are skipped or replaced.

The useful workflow is to export a multi-package source project, move the export
away from the original checkout, then use ordinary `check`, `run`, `test`,
`build` and native `package` commands on the new root `oxid.toml`. The latter
commands retain their existing entry, host, toolchain and resource requirements.
An installed compatible compiler is still required; native tools and system
libraries are not included. This is a partial vendor/offline-source workflow,
not registry support, an integrity lockfile, a reproducible compiler-build claim
or a completed G5/v1.0 gate.

## Exact retained scope

Only the selected package and its resolved transitive dependency closure are
included, not unrelated workspace members. Source files are copied byte for byte
from the already checked session's retained source map. They are not reopened
from potentially changed source pathnames after checking. Public and private
sources, function bodies and comments are all retained.

The export is NOT a lossless project backup. Original manifest comments,
formatting and version metadata are not copied. Scripts, undeclared source
files, assets, lockfiles, build artifacts and workspace configuration are not
included. The command does not discover or infer such files. Literal strings
and comments inside retained source bytes are not edited or redacted.

Generated manifests preserve explicit `edition = "typed-preview"`, each
nonempty retained project name, and dependency aliases and topology. Other
metadata is intentionally omitted. Names use the escapes accepted by the
existing manifest decoder: quote, backslash, newline, carriage return, tab and
four-digit Unicode escapes for remaining control characters. Other Unicode is
retained as UTF-8. Host source/manifest path metadata is not exported.

## Deterministic layout

Package zero is the selected package and occupies the output root. Remaining
packages use `packages/pN`, where N is their existing declaration-DFS graph
ordinal. Shared dependencies are emitted once, while distinct packages with
same-spelled names and types remain distinct. Package names are metadata, not
output directory names.

Each package has an `oxid.toml` and a `src` directory. Its root source is named
`src/.root.ox`. Other sources retain their module-relative paths under `src`.
The hidden root filename is not a valid declared module identifier; duplicate
or file/directory-prefix collisions are nevertheless explicitly rejected.
Source syntax and import paths are unchanged.

A root package dependency on package N becomes `packages/pN`. Dependencies
between non-root packages use `../pN`. Aliases retain declaration order.
Generated file order is package DFS order, manifest first, then that package's
module DFS order. No timestamp or absolute host path is added to generated
manifests. Identical checked package topology, names and source bytes yield the
same relative paths and file bytes, independent of the destination name.

Planning validates graph/module ranges, file identities, dependency targets,
relative paths and collisions before output I/O. Generated manifests together
must fit the existing 16,384-byte aggregate manifest input limit. Export-relative
file paths are limited to 4,096 bytes each and 4 MiB combined, with fallible bulk
allocation. These are output restrictions, not changes to compiler admission.
Existing loader limits still apply when checking the relocated tree, including
limits affected by its caller-chosen absolute destination path. Export failure
never raises any source, manifest, ownership, runtime or native ceiling.

## CLI boundary

`--manifest-path` must follow `export`; `--package` selects a workspace member.
`--output` is required; relative paths are resolved from the caller's current
directory. Split and `=` values are accepted. Explicit `--edition typed-preview` and
`--message-format text` are accepted. Duplicate/missing/empty values, legacy
edition, JSON, source operands, profiles, engine, entry mode, backend, target and
experimental compiler selectors are rejected before manifest I/O. `--` ends
option recognition; it cannot turn later arguments into selectors. Options
belonging to a manifest script remain that script's arguments.

## Fresh output and failure contract

The destination must be a new directory with an existing parent. Existing files,
directories and symlinks, including dangling symlinks, are refused. Output paths
with traversal or symlink ancestors are rejected. Entry paths are relative,
forward-slash paths with no empty, `.` or `..` component, backslash, NUL or
control character. Every entry path and all duplicate/prefix collisions are
validated before the destination is created. Files use exclusive creation;
existing content is never overwritten or merged.

If writing fails after the root directory is created, partial files and
directories are retained for inspection. The error explicitly distinguishes
whether this invocation created the destination. A successful write flushes all
files. This is not a whole-tree transaction, a crash-durability guarantee, a
sandbox or protection against arbitrary noncooperating concurrent filesystem
mutation; stable filesystem contents are assumed. A failed invocation must be
retried at a fresh destination, or after the user independently inspects and
removes the partial output.

Success is one text status line and exit status 0. Loading, checking, planning
or writing errors go to stderr and return 1. No success line is emitted for a
failed write. If the status stream fails after export, the command returns 1
and attempts a stderr diagnostic explicitly saying the complete destination
remains; it does not undo completed output. No native build ownership receipts
or existing artifacts are read or changed by this command.
