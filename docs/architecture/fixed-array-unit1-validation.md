# Fixed scalar arrays: Unit 1 identity/layout groundwork

This ledger describes private groundwork for [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md).
The proposed source capability remains disabled. There are no executable raw
array values, constructors, indexing, length operations, or changed source
selectors in this unit. Existing record consumers now query the common checked
aggregate layout/width seam; record type validation uses the same facade.

Baseline is commit `0ef3be1df3643febdff1f859a4eb1ce567ab8164`, tree
`24bfdb84b042899f655e5dab472d2ad8be24c762`. Source changes are restricted to
`owned_types.rs`, `owned/plan.rs`, a comment correction in `owned/mod.rs`, and
one new test module, `owned_types/array_tests.rs`, under `src/frontend/oir`.
The added Rust source file changes source-inventory cardinality by one. Frozen
historical source authorities and qualification manifests are not retargeted
by this change; current-source binding integration is separate work.

## Checked interface and gating

`FixedArrayTy::check` accepts bool/i32/unit and length 0..1024, checking before
conversion to its private u16 field. `AggregateTy` is explicitly tagged;
arbitrary nominal RecordIds are never interpreted as arrays. The declaration
facade validates nominal IDs before layout, width or equality. Equality checks
actual then expected, including when two invalid IDs are equal. Array metadata
contains no source origin, persistent table or supplied size/stride.

Layout uses scalar size/alignment 1/1 for bool/unit and 4/4 for i32, checked
multiplication/alignment, and positive aligned storage for zero elements. Zero
i32 arrays occupy 4 bytes with width 1; bool/unit zero arrays occupy 1 byte with
width 1. These are type/layout calculations only. Runtime initialization,
transfer, indexing, capability and fuel behavior remain later implementation.

ValueTy, ParameterTy, raw owners/references/loans and both consumer storage
representations remain record-only. No conversion to a verified executable
array witness exists. Existing verifier usage accounting is still record-only;
this unit does not claim all ownership accounting is generalized.

## Measured representation and resource evidence

Measured on Linux x86_64 with Rust 1.99.0 (`b940084d7`). New ephemeral descriptors
are FixedArrayTy **4 bytes**, AggregateTy **16 bytes**. They are not embedded in
retained source, HIR, raw ownership, declaration, plan or runtime storage.

| Existing representation | Measured bytes |
| --- | ---: |
| RecordId / FieldId | 8 / 16 |
| ValueTy / ParameterTy | 16 / 24 |
| RawRecordDecl / RawFieldDecl | 56 / 64 |
| RecordDecl / FieldDecl / Declarations | 64 / 56 / 80 |
| DeclarationError | 32 |
| AST Program / Function / BodyBlock / ItemId | 248 / 200 / 72 / 16 |
| AST Expr / Stmt / TypeSyntax | 88 / 136 / 64 |
| RawOwnedProgram / RawOwnedFunction | 48 / 248 |
| OwnerDecl / ReferenceDecl / LoanDecl / CallDecl | 56 / 48 / 72 / 96 |
| OwnedInstruction / ParameterBinding | 128 / 16 |
| FunctionPlan / CallPlan / FrameUsage | 184 / 48 / 88 |
| OwnerRuntime / ReferenceHandle / LoanRuntime / CallRuntime | 32 / 64 / 96 / 16 |

The empty-function AST bound is still `200 + 72 + 16 = 288` bytes per counted
node. The existing broader per-node inventory test remains applicable. Checked
declaration payload stays `64R + 56F`: at R=4096/F=65536 it is **3,932,160**
bytes. Counts and the separate 8 MiB declaration payload, 1 MiB declaration
layout, 32 MiB ownership metadata/scratch and 64 MiB source raw ceilings are
unchanged. New queries allocate nothing and use O(1) scratch/work; repeated
structural queries do not grow declaration counts/capacities or payload.

A retained comparison checks 35 persisted type declarations byte-for-byte
against the base, plus six entire unchanged AST/storage/budget/verifier source
files (41 comparisons). This unchanged-definition evidence complements actual
measurements; it is not a claim that Rust layouts are universally stable. Every
later persisted carrier/AST migration requires new measured formulas and
compatibility review. No new allocation was introduced, so there is no new
array allocation-failure surface to claim tested.

## Focused evidence

The initial focused test compilation failed with missing FixedArrayTy,
AggregateTy and aggregate-query methods (exit 101), establishing the missing
interface. After implementation:

- Eight new declaration-seam tests pass, including all **3075** scalar/length
  combinations, closed-form layout/width expectations, structural/nominal and
  zero-length identity, invalid-ID precedence, 1025/65536/usize::MAX rejection,
  repeated queries, measured representation, and seven source-syntax rejections
- The complete owned_types suite passes **38** tests, including predecessor
  declaration count, padded-layout, malformed-ID and lowered-cap controls
- A separate real-crate raw/plan/runtime representation test supplies the sizes
  above and checks the current qualified 64-bit representation

These are nine added Rust tests, not 3075 new programs or native execution
cases. Seven parser rejection fixtures do not constitute array source support.
Ordinary gates are recorded below; independent component results are separate
from these real-crate tests.

Commands use a fresh exclusive CARGO_TARGET_DIR, `CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=2`, the locked dependencies and pinned Rust 1.99.0. Focused
commands are:

```text
cargo test --locked --bin oxid frontend::oir::owned_types::array_tests -- --nocapture
cargo test --locked --bin oxid frontend::oir::owned_types -- --nocapture
cargo test --locked --bin oxid aggregate_seam_retains_raw_and_runtime_representation -- --nocapture
```

## Ordinary qualification on the frozen source

All commands below exited 0 on the same source bytes:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Passed |
| `cargo test --all-targets --all-features --locked` | 798 passed, 0 failed, 21 ignored |
| `cargo test --release --all-targets --all-features --locked` | 798 passed, 0 failed, 21 ignored |
| `cargo build --release --locked` | Passed |
| `python3 scripts/verify_owned_witness_privacy.py` | 22 probes passed |

Each complete Rust profile ran 17 executables: 658 unit and 140 integration
successes. The 21 previously ignored tests remain unexecuted by these commands.
The nine added tests are included, not additional to those totals. Existing
record layout/plan/fuel, allocation-failure, native-emission, source routing,
legacy and syntax/resource regressions execute through the normal suites.

Qualification source commit was `826b86532ff393cb01d1c6b3a77d193b42039eb7`,
tree `de0765c4178cdf23d4fca69f67302565c12225de`. The documentation was separately
published at `4aebb4b2d7edde1af2a683e041325d246dc602fe`; rebasing the bounded
implementation onto that identical documentation tree produced
`ffdf214cf40be685a5c06263ef458818879c2264` with the identical complete tree and
all four Rust source hashes unchanged. Later validation-ledger edits do not
change qualified production bytes. No unchanged test is claimed freshly rerun
solely because Git ancestry or this report changed.

The exact qualified Rust SHA-256 values are:

- `owned_types.rs`: `f9b166ce29af411cf6babe3d3ea7ded46607a335902b54b540eaeeb2e6baa67a`
- `owned/plan.rs`: `765fc8539d90cae8077a142141da9628b501a5f04731d5442bcf5a119a8fd318`
- `owned/mod.rs`: `bf170de2c7987e528305b061143bf7188b8e3b90d02876fe50676fff92759a03`
- `owned_types/array_tests.rs`: `ab62e471a0a4edcbe054f6df5acb0b26ad5db9cb28037d6093e456703bc09cca`

These are Linux local results, not exact-head hosted CI. No ignored LLVM corpus,
additional host execution, source-array pilot qualification or public array
activation is claimed. Current-source binding integration remains separate from
these source changes and must retain historical authorities unchanged.


## Independent review

An independent reviewer passed the bounded seam on the exact frozen source tree
and source hashes above. It independently reconstructed both published-document
and implementation checkpoint trees and rechecked all 41 unchanged-definition
comparisons. Its component harness compiled the byte-identical production
owned_types module with explicit scalar/source-span shims in unoptimized and
opt-level=3 builds. The shims do not implement aggregate behavior and are not
used as evidence for real-crate representation sizes.

Both independent runs passed all 3075 scalar/length pairs on empty/populated
tables, 21 invalid lengths, structural and nominal/cross-kind identity,
malformed-ID precedence, known mixed/empty record layouts and legacy descriptor
validation. The query/control loop observed zero allocations. Separate sibling
compile-fail probes rejected direct array-field construction (E0451) and mutation
(E0616). This is component/layout evidence, not runtime array execution.

The reviewer separately inspected actual-crate size output, counted both ordinary
Rust profiles from their raw streams and verified the debug/release binary hashes.
The retained report SHA-256 is
`3484653c8c7c5f03dc14488417e511bc089342f15fcf7a15a848bd2aefe8cf65`.
No material implementation finding remained. The successor recovery payload
clarifies the external pinned toolchain environment path without changing source.
