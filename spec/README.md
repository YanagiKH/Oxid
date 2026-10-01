# Specification baseline

This directory begins the M0 specification work. It is not a complete language
specification or evidence that M0 has been completed.

- [Legacy 0.9 behavior](legacy-0.9.md) characterizes selected existing behavior
  through the public CLI, both as source and as an OXBC artifact.
- [Legacy module contract](legacy-modules.md) records module resolution,
  diagnostics, and route-specific limitations.
- [Current architecture](../docs/architecture/current-baseline.md) identifies
  which implementation actually runs and separates demonstrations from it.
- [Feature status](../docs/feature-status.json) records scoped claims and gaps.
- [RFC process](../rfcs/README.md) describes how new semantics are introduced.

## Version boundaries

The runtime/toolchain release is currently 0.9.0. Its Rust crate edition (2021)
selects Rust syntax for building the host implementation; it is not an Oxid
language edition or a claim of Rust language compatibility.

"Legacy 0.9" is the name of the characterized behavior in these documents. There
is currently no Oxid edition selector. Adding one requires a migration design;
the proposed static core must not silently replace existing numeric, equality,
aliasing, or task behavior.

The production container uses OXBC format 1.0 and serialized AST version 1.
`.oxa` currently contains the same representation as `.oxb`. These are neither
a native target ABI nor an executable instruction-set specification. Lockfile
version 1, toolchain release version, artifact version, and language edition
must be tracked independently. See [the current artifact format](../docs/COMPILER.md).

## Still open

A complete grammar, binding/scope and initialization specification, new static
numeric rules, ownership/aliasing, pointer provenance, initialization/destruction,
panic, concurrency, ABI, and edition migration contracts remain to be designed
and tested. Characterization of the old interpreter is not an oracle for these
new semantics. No memory-safety or Rust-compatibility certification is implied.
