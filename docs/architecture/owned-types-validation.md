# Private owned-type declaration groundwork validation

This report covers only the immutable declaration/identity/layout facade in
[RFC 0014](../../rfcs/0014-owned-structs-call-borrows.md). Baseline: PR17 merge
`f8061f403415fd728dc8beee73175a0b6b5e4e20`, tree
`07c5f1d998d444baecfef8b53deb252ecd8c62ef`.

## Scope

The production-private facade checks nominal record and record-qualified field
identities, scalar-only field types, source spans and fixed layouts before owning
flat immutable tables. Only its module registration and implementation/tests
change production source. The existing scalar type/value representation,
VerifiedProgram, source parser, reference executor and LLVM backend do not change.
The new facade has no production caller; its module-local dead-code allowance is
explicitly temporary. It is groundwork, not a delivered ownership feature.

Struct declarations/literals and reference parameters/borrow arguments still
fail source parsing. No move/loan safety, aggregate execution, new native target,
heap cleanup, M2 completion or Rust-level safety result is claimed.

## Local gates

Host: Linux x86_64; Rust/Cargo 1.98.1. Fresh complete debug and release suites each
passed 429 ordinary tests: 300 unit plus 129 integration, zero failures. Four
pre-existing Linux LLVM tests are ignored by those commands and are not counted
as executed native gates here. Both profiles passed all 25 new focused tests.
Formatting, strict all-target/all-feature Clippy and git diff checks passed. After public documentation integration, all seven metadata tests and repository verification of 121 sources/67 runnable programs passed; the feature inventory remains at 24 entries.

An initial focused build failed with E0583 because only the new module
registration existed. This proves missing groundwork, not an independently
observed failing assertion for every final test. The later focused suite passed.
A release build interrupted before completion was rerun to completion; only the
completed result is evidence.

Focused coverage includes:

- Empty/scalar layouts, declaration order, alignment/tail padding and nominally
  distinct identical layouts
- Duplicate/out-of-order/huge IDs, wrong-record field IDs, bad UTF-8/source spans,
  nested/borrowed field rejection and checked lookup/type descriptors
- Immutable ownership of checked tables with no alias to mutable raw inputs
- Separate owner/loan/call IDs and unchanged scalar display/equality/JSON behavior
- Inclusive and one-over record/per-record-field/aggregate-field limits, table
  bytes and padded-layout bytes, zero limits and clamped lowered-limit testing
- Checked usize multiplication/addition/alignment overflow; limit preflight before
  malformed field traversal; unchanged scalar resource constants and source gate

## Resource evidence

R records/F fields require requested payload
`R*sizeof(RecordDecl)+F*sizeof(FieldDecl)`. The checker performs one count
preflight and two linear validation/construction traversals. Persistent space is
O(R+F), scratch O(1); no CFG analysis, recursive traversal, runtime activation or
blocks-by-owners matrix is introduced. Two fallible flat-vector reservations
follow all declaration validation, byte limits and checked layouts.

On this host RecordDecl is 64 bytes and FieldDecl 56. A fixture with all 4,096
records and 65,536 fields requests 3,932,160 table bytes. It has 64 full 1,024-i32
records plus 4,032 empty records: 266,176 total layout bytes. Measured vector
capacities were exactly 4,096/65,536. An isolated test child reported peak RSS
12,608 KiB including raw fixture/output/test-process overhead; this is one measured
fixture, not a universal memory bound.

The 8 MiB table and 1 MiB layout ceilings are redundant under current counts and
measured representation; lowered-limit cases still exercise exact admission and
one-over failure. Caller-owned raw input, source storage, headers and allocator
overhead are outside requested payload. Runtime/fuel/loan-analysis bounds remain
future work.

## Independent verification

A separate verifier reviewed the frozen two-file production change and used an
isolated copy with six held-out tests. In both debug and release it checked
88,573 mixed scalar layouts, 864 source-span cases, 324 field-ID placements,
216 lookups and 65,664 alignment results against u128 arithmetic. It also
checked rejection before allocation for malformed inputs, exact/one-over caps,
and both fallible output-allocation failures with cleanup. No material defect
was reported. Each copied-crate suite passed 435 tests (the candidate's 429 plus
six held-out tests), with the same four existing LLVM tests ignored. Formatting
and strict Clippy passed against exact source. These are bounded validation
families, not proof of future move/borrow checking or every program.

## Source identity and reproduction

Stable SHA256 values for the reviewed production source:

- src/frontend/oir/mod.rs: 5a6e9000381b987d63ab160addb9057fb5024cd7e22e580b582f0ac1b42ace70
- src/frontend/oir/owned_types.rs: c5b50d3fe0592bcc6a2d98d54fb552f7a402ad9ed9dcf3a2f50d50d92e5c79f2

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --bin oxid frontend::oir::owned_types
cargo test --release --locked --bin oxid frontend::oir::owned_types
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
```

Candidate CI is separate qualification evidence, recorded with the exact candidate commit. Independent review above used the exact frozen production hashes. The already successful PR17
post-merge run is predecessor evidence and does not qualify this change.
