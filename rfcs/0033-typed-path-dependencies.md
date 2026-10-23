# RFC 0033: local packages in a typed compiler session

Status: experimental implementation, Linux package loading.

## User outcome

A typed project can call public functions and use owned records or enums from
local dependency libraries through ordinary `check`, `run`, `build`, and `test`.
The compiler checks the entire graph in one session. The byte-count application
in [apps/byte-count](../apps/byte-count/README.md) exercises a shared dependency,
binary stdin/stdout and source-free native deployment.

This extends [RFC 0032](0032-typed-project-workflow.md). Existing explicit file
commands, single-package source semantics, ownership rules and native contracts
retain their previous behavior. Typed Git/registry resolution, lockfiles,
workspaces, packaging and incremental compilation remain future work.

## Manifest and graph identity

`[dependencies]` maps an alias to a local directory path string, relative to the
declaring manifest or absolute. Each dependency contains `oxid.toml`, explicitly
selects `typed-preview`, and supplies a valid project name. Names are metadata;
two directories may have the same name. Dependency roots default to `src/lib.ox`,
or use their own explicit `[project].entry`. They need no `main` function.

Canonical directories identify packages within this session. Repeated paths,
aliases and diamond dependencies to one directory load one package. Different
canonical directories have distinct declaration and nominal type identities,
even when their names and source text are identical. This is local identity,
not a registry version resolver or a source-integrity lock.

The graph rejects cycles, absent manifests/sources, unsupported dependency
forms, mixed or absent dependency editions, invalid aliases and unsupported
build settings before program effects or output publication. Aliases are ASCII
identifiers of at most 64 bytes; reserved words and case-folding collisions in
one manifest are rejected. A dependency alias cannot exactly match a function,
type, module or import alias declared in that package's root source.

Graph limits are 256 packages, 1,024 dependency edges and maximum dependency
path depth 32. Shared tails count toward the complete path irrespective of DFS
order. The existing total source-byte, AST, work and 256-module limits apply
across the whole session. Entry files remain inside their package directories;
entry/module symlinks and overlapping source identities are rejected. Source
trees are trusted stable inputs during compilation, as in the existing loader.

## Name resolution and safety

The existing grammar represents a dependency import as
`use crate::math::answer;`. `math` is consulted as a dependency alias only at
the first component after `crate::` in the requesting package. The remaining
path uses normal visibility and declaration resolution in the target package.
Inside a library, `crate::` starts at that library's root. An application cannot
traverse another package's private dependency aliases to acquire an undeclared
dependency; a library can expose public functions using its dependencies.

Source files remain unchanged. A package sidecar records a forest of module
roots and their graph edges around one source map, parser output set and sealed
declaration index. Definition, record and enum IDs are global to this session.
The existing ownership checker, verified OIR and native lowering consume these
identities. No synthetic source declarations, concatenation or legacy fallback
participate in a typed package build.

Private items and fields retain their declaring package/module domains. Calling
a library does not relax move, borrow, field visibility, array bounds, overflow
or execution-fuel rules. Only the application's root supplies the process or
result entry; a dependency's function named `main` is an ordinary function.

## Commands and tests

Source-less project commands select the manifest graph. Inferred file commands
in a project with dependencies report guidance to use a project command;
explicit `--edition` file commands keep the original standalone route.

Each declared `[tests]` entry runs in an isolated process with closed stdin.
It retains the same package graph and replaces only the application entry.
The child invocation selects a declared test name; it cannot supply an arbitrary
entry path. Successful execution returns zero and a failing test fails the
suite. Managed build publication remains atomic after successful compilation
and linking; failed builds preserve the previous executable.

Package loading/checking and result execution are currently qualified on Linux.
Process execution and native output remain Linux x86_64; native compilation
requires LLVM/Clang/LLD 19.1.7. The five-host compiler test matrix also checks
explicit rejection on unsupported package hosts.

## Validation scope

Compiler and public CLI tests cover transitive roots, diamonds, distinct nominal
identities, private declarations/fields, ownership failures, cycles, aliases,
graph bounds, missing/invalid manifests and sources, test entry selection and
managed native output. The application oracle compares independent newline and
byte counts over empty, boundary, binary and multi-chunk inputs in both execution
paths, then edits a dependency, checks failure preservation and deploys the ELF
after removing source files.

The current Unit2 wrapper explicitly admits these package-boundary changes but
reuses unchanged assertions for predecessor single-package behavior and resource
counts. Those results do not qualify the new package semantics; the separate
compiler, CLI and application checks supply that evidence. Historical source
certificates remain bound to their historical commit. Neither this increment
nor these tests complete R1 or any v1 release gate.
