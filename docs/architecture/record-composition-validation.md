# Bounded record composition: local validation

This experimental implementation follows [RFC 0020](../../rfcs/0020-owned-record-composition.md).
It is not a completed M2/M3 milestone, Rust compatibility claim, stable ABI or
proof of memory safety. Existing historical reports retain their original
source/compiler scope; the current source-binding successor and exact-head
hosted CI are separate required steps.

## Baseline and checkpoints

The published base is PR33 head `848b36bcb2e23a1560d6a3f7e84402818b78d3a4`, tree
`f8e2d39d8081b992969597649bad7fd9f28e7d04`. Local work used the identical tree at
`4c1c040556b216d5d044a029ed6f05825d446d76`. The declaration-only checkpoint was
published as `72895809758312c3524f7e3b588f6124bebe928e`, tree
`3a0774d60efda1b4b18ffd0da8d76dd453ca2da7`; it explicitly kept executable record
composition closed. That checkpoint passed 47 declaration tests and 43 raw
ownership tests, with one existing ignored test.

## Observable acceptance

- The Meta/Batch pilot moves a record and fixed scalar array into one outer
  owner, relays the complete Batch through a parameter/result, reborrows its
  complete root exclusively, and inspects it through a shared helper
- Starting with samples [1,2,3] and completed=0, the loop produces [2,3,4] and
  completed=3. Reference and source-free Linux ELF execution return 324
- The [checked-in three-module sample](../../fixtures/typed-record-composition-samples/README.md)
  also returns 324 and is included directly by the public acceptance test
- Constructor expressions execute once in written order, independently of field
  layout order; earlier child moves are visible to later initializer expressions
- Scalar/contained-array access enforces privacy at every field hop, whole-root
  availability, mutable/exclusive permissions and explicit reborrow suspension
- Aggregate extraction/replacement, projected borrows/slice conversion, partial
  ownership, stored references, nested arrays and contextual empty-literal
  inference remain rejected

## Independent raw and consumer checks

The raw pilot has an independently hand-counted 269-fuel schedule. Reference
and native runs agree at every budget from 0 through 269, including the first
unpaid source span. This raw cost is not a claim about source lowering costs.
The native harness runs emitted ELF with a cleared environment and no compiler
on PATH. LLVM/Clang/LLD 19.1.7 at O0 was exercised using debug and release builds.

Declaration controls cover forward references, direct/mutual/disconnected
cycles, exact depth64 and depth65 rejection in both declaration orders, compact
exponential containment graphs, checked width/padded layout sums, empty records,
zero arrays, nominal path mismatches, invalid continuation past scalars/arrays,
and bounded lazy leaf traversal at maximum depth.

Raw verifier controls cover missing/duplicate fields, duplicate owned initializer
consumption, unstaged sources, type mismatch, invalid IDs, aggregate reads through
legacy scalar instructions, whole-root moves and permission failures. Reference
fault controls independently corrupt root activation/generation, permission and
nominal identity, and verify constructor preflight atomicity, initialized
sentinels, no padding copying, and fuel-before-bounds/no-store behavior.

Eleven new native composition groups pass in both debug and release, including
24 distinct retained LLVM/ELF artifacts per profile, maximum-depth source
construction/relay/projection, projected bool/unit mutation, guarded and acyclic
empty storage, RHS/index snapshots, projected boolean phi successors, bounds
failure origins, every fallible native allocation and emission limits. Existing
native regression filters pass 80 tests with 21 explicitly ignored native gates;
existing reference filters pass 37 array tests and 43 reference-consumer tests.

## Reproduction and scope

At the integrated source checkpoint, the locked all-target/all-feature Rust
suite passed 1,105 tests with 43 explicit ignores. All-target Clippy with warnings
denied and full formatting checks passed. The public native gate passed in both
profiles; repository verification passed 134 language sources, 126 checks and
72 runnable programs. Formatter goldens passed 36 positive and 21 negative cases.
The newly admitted array-record-field syntax supersedes its former negative;
other historical goldens remain unchanged.

The Python registration controls preserve historical inventory fingerprints by
subtracting only the exact three new sample members. The current source-binding
admission deliberately rejects the changed compiler until its separately reviewed
successor is published; no historical binding is silently weakened.

Use the repository's qualified Rust toolchain and pinned LLVM toolchain:

```sh
cargo test --all-targets --all-features --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --locked --test typed_record_composition -- --ignored --test-threads=1
cargo test --release --locked --test typed_record_composition -- --ignored --test-threads=1
cargo test --locked --bin oxid native_composition -- --include-ignored --test-threads=1
cargo test --release --locked --bin oxid native_composition -- --include-ignored --test-threads=1
```

The explicit native CI step registers these debug/release public and internal
gates; registration is not evidence of a hosted pass. Native qualification is
Linux x86_64 only and retains the ELF loader/libc dependency. Whole-record
ownership remains the existing sealed-witness discipline, with no new subobject
owners, alias permissions, lifetime inference or general aggregate arrays.
