# RFC 0037: native toolchain diagnosis

Status: implemented for the current bounded Linux x86_64 native target.

## Problem

The historical `doctor` command checks project paths and manifest structure.
Those checks do not establish that an installed LLVM compiler, target SDK,
linker, target libraries or native loader work together. A file existing on
`PATH`, or even a successful version command, is insufficient evidence.

## Command and compatibility

Add explicit `oxid doctor --native` and `oxid diagnose --native`. The existing
commands without `--native` keep their previous behavior. Native diagnosis
accepts optional `--edition=typed-preview` and `--message-format=text|json`.
The supported target remains `x86_64-unknown-linux-gnu` and the required LLVM
version remains 19.1.7. Other native compiler hosts fail the host check rather
than claiming cross-compilation support.

Tool resolution, version matching and runtime compilation arguments are shared
with the production native boundary. SDK diagnosis compiles the production C
runtime. The full probe invokes the running Oxid compiler on fixed source using
the ordinary uncached explicit-file compile route, verifies its ELF target and
executes it with an exact exit/output oracle. Probe failures cannot publish a
project artifact or mutate a lock.

Typed project diagnosis reuses manifest admission, canonical local package graph
construction, retained-source preparation and typed lock verification. It parses
the selected application's loaded source closure, but does not run or type check
user programs, scripts or tests. Missing locks are optional and reported as such.
Legacy manifests retain the historical project doctor path.

## Bounded probes and output

[Native doctor documentation](../docs/NATIVE_DOCTOR.md) specifies the versioned
JSON schema, check IDs, exit statuses, deadlines, capture and address-space
limits. The command has no network operation. It writes only private temporary
probe storage and cleans it on success or failure. Tool processes are trusted
inputs; bounded diagnosis does not create a sandbox for hostile executables.

## Evidence

`tests/native_doctor.rs` exercises option admission, legacy compatibility,
manifest and source failures, stale and malformed locks, relative and PATH tool
selection, wrong version banners, terminal control characters, excessive output
and hung process groups. Its explicit LLVM integration uses the real 19.1.7
compiler for successful compilation/link/execution, missing SDK and target
library failures, and an output flood inside the compiler child's tool tree.
Existing native artifact, edition, manifest, project and lock regression suites
remain unchanged.

This increment diagnoses the current native target. It does not establish a
stable language ABI, release readiness, additional SDK targets, workspace
semantics or complete roadmap R1 acceptance.
