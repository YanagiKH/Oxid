# Typed project Batch example

This example separates an opaque move-only Batch record and its operations into
`state.ox`, a job helper into `jobs.ox`, and the original entry function into
`main.ox`. It uses declaration-only modules, direct imports, visibility, whole
moves, call-only shared/exclusive borrowing and loops. Private fields remain
inside the state API. The source-derived result is **816** across eleven functions.

Run from the repository root with the current experimental compiler:

```sh
oxid check fixtures/typed-project-batch/main.ox --edition typed-preview
oxid run fixtures/typed-project-batch/main.ox --edition typed-preview
oxid compile fixtures/typed-project-batch/main.ox --edition typed-preview \
  --backend llvm --output /tmp/oxid-typed-batch
/tmp/oxid-typed-batch
```

The output path must not already exist. Declared-child loading admits Linux;
measured native support is Linux x86_64 with LLVM 19.1.7 at O0. Native compilation
does not run its output. A successful native program prints `816` and needs no
Oxid runtime or source files, while retaining its host libc/ELF-loader dependency.

Exact source identities and independent arithmetic are in
[semantic inputs](semantic-inputs.json). Source qualification, executed native
coverage and platform limits are recorded in the
[Unit4 ledger](../../docs/architecture/typed-project-unit4-validation.md).
The original [single-file pilot](../owned_source/batch.ox) and mechanical split
are separate cost-model cases; equal result does not imply equal fuel cost.
No filesystem I/O, scheduler, heap containers or stored references are added.
