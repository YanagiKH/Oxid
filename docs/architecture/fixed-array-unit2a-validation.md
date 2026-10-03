# Fixed scalar arrays: gated Unit 2A identity carriers

This is the first preparatory slice of [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md)
Unit 2. It does not implement executable arrays or activate array source syntax.
The published base is `d7627f65bec1e5019291763245352ab40e5d206f`, tree
`ca7aa096649989a4a65d47a63699e9914165702c`. The coherent implementation checkpoint
is `87b4fcf7a3889177a5480c66d6b691f424c375a8`, tree
`a3d0dad5cfd3d3928d51a7e81fee807b8cb36f8a`.

## Interface and executable admission

`ValueTy::Owned` now contains semantic `AggregateTy`; reference parameters use
`ParameterTy::Reference { aggregate, kind }`. A new `AggregateSlot` retains the
identity in owner, reference, loan and diagnostic-subject rows. Its private
representation has an independent explicit tag and either a u32 nominal ordinal
or the checked fixed-array descriptor. It introduces no type table, interning,
synthetic record, packed RecordId bits, allocation or execution witness.

`AggregateSlot::try_from_aggregate` is checked and fallible. `aggregate()` decodes
the semantic identity without validating it. Raw nominal ordinals 4096 through
u32::MAX remain representable and invalid; declaration queries still validate
actual then expected before equality. Shape equality maps invalid IDs and type
mismatches to each site's existing category and origin. In particular, an
invalid borrowed-call loan identity remains `Malformed::Binding` at the call
span, after wrong-call/argument checks and before later loan validation. Existing
runtime invariant labels and record diagnostic text are retained.

The sole representation exception is constructing a retained carrier from a
RecordId above u32::MAX. Construction now returns `InvalidRecordId` with the
original full ID immediately. No valid record is removed: the unchanged valid
record-count cap is 4096. Full-width semantic descriptors and raw record/field
identities remain available. Existing malformed-owner fixtures use representable
invalid ordinals; the work-before-validation fixture still observes its original
resource failure. Separate tests cover the earlier full-width construction error.

The authoritative `verify_with_limits` path explicitly rejects every fixed-array
result, owner, reference and loan after unchanged resource preflight and checked
record declarations, before signature checks or witness creation. This includes
unused declarations, unused parameters, result-only inactive functions and
infinite-loop functions with no array operation. Raw array-valued record fields
remain rejected by the earlier non-scalar-field declaration check. Therefore no
array carrier can reach either consumer or an execution plan. This gate must
remain until all verifier and reference/native array support lands together.

The additional admission scan allocates nothing. Retained rows each charge one
visit to the existing ownership meter; O + R + L is at most the existing N term.
They fit the fixed-pass allowance in `(4O + L + C + 32) * N`; no admission formula
or cap is changed. Function-result inspection is ordinary signature inventory,
bounded by the existing program function cap even when ownership work is zero.
Scalar CFG verification, shape/flow rules, runtime storage and all count/byte
ceilings retain their earlier boundaries.

## Actual representation

Measured in the real crate on Linux x86_64 with Rust 1.99.0, with assertions
against the previous 64-bit envelopes:

| Representation | Bytes |
| --- | ---: |
| AggregateSlot / AggregateTy | 8 / 16 |
| ValueTy / ParameterTy | 16 / 24 |
| RecordId / FieldId / FixedArrayTy | 8 / 16 / 4 |
| RawRecordDecl / RawFieldDecl | 56 / 64 |
| RecordDecl / FieldDecl / Declarations / DeclarationError | 64 / 56 / 80 / 32 |
| AST Program / Function / BodyBlock / ItemId | 248 / 200 / 72 / 16 |
| AST Expr / Stmt / TypeSyntax | 88 / 136 / 64 |
| RawOwnedProgram / RawOwnedFunction | 48 / 248 |
| OwnerDecl / ReferenceDecl / LoanDecl / CallDecl | 56 / 48 / 72 / 96 |
| OwnedInstruction / ParameterBinding | 128 / 16 |
| FunctionPlan / CallPlan / FrameUsage | 184 / 48 / 88 |
| OwnerRuntime / ReferenceHandle / LoanRuntime / CallRuntime | 32 / 64 / 96 / 16 |
| DenialContext / DenialFacts / OwnedFailure | 136 / 136 / 240 |

These are real-crate measurements for this host, not independent measurements
for other hosts. Existing byte ledgers still charge the same row sizes. The
source parser, AST, ownership/source budget implementations, scalar CFG adapter
and runtime storage implementation are unchanged from the published base.

## Focused validation at the implementation checkpoint

Five new Rust test functions passed:

- One tests 54 array-carrier combinations and zero execution-plan/reference/native
  consumer entries; these are nine element/length pairs over six carrier cases
- One retains preflight/declaration priority and non-scalar-field rejection
- One tests malformed record category/origin parity, including six competing
  borrowed-loan defects and full-width result descriptors
- One checks 3075 fixed-array storage roundtrips and semantic value/reference
  queries, plus nominal roundtrips and representable malformed-ID rejection
- One rejects u32::MAX+1 and usize::MAX at construction while preserving full IDs
  through ordinary semantic queries

The existing raw/plan/runtime representation test was extended, not counted as
a new test. The existing declaration/source size inventory, seven source-array
syntax rejections, inclusive/lowered resource ceilings and priority, and exact
record provenance/fuel/native-output control also passed. Formatting and
whitespace checks passed. These loop counts are input
combinations, not separate test functions or executable array programs.

## Ordinary validation

On the same implementation bytes, with an exclusive target directory,
`CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2` and pinned Rust 1.99.0:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Passed |
| `cargo test --all-targets --all-features --locked` | 803 passed, 0 failed, 21 ignored |
| `cargo test --release --all-targets --all-features --locked` | 803 passed, 0 failed, 21 ignored |
| `cargo build --release --locked` | Passed |
| `python3 scripts/verify_owned_witness_privacy.py` | 22 probes passed |

Each Rust profile ran 17 executables: 663 unit and 140 integration successes.
The five added test functions are already included in each total. The 21
previously ignored tests were not executed by these commands. All commands
exited zero; their source hashes remained exactly those of the implementation
checkpoint throughout validation.

## Independent bounded review

An independent reviewer passed the exact implementation checkpoint with no
material finding. The baseline's complete tree was the published base tree
`ca7aa096649989a4a65d47a63699e9914165702c`. All 33 changed source files matched
the frozen candidate inventory; the other 991 snapshot members were unchanged.
Actual-crate held-outs ran 11 baseline and 14 candidate test functions, using
immutable source/observer identities emitted by the executables, checked full
member inventories, fresh compilation and saved binary hashes.

The comparison retained byte-identical outputs for 141 malformed-record
failure rows, 66 existing layout/alignment rows, four resource-usage rows,
13 JSON diagnostics and five complete human diagnostics. It included a
128-case shared/exclusive loan-defect matrix. Forty source-array syntax
rejections per run, 72 candidate raw-carrier admission cases, 3075 structural
roundtrips and explicit admission/construction exceptions passed. These are
reviewer-owned input loops within the test functions, not executable arrays.

Five additional component compile probes checked the exact production slot
module's visibility and fallible conversion, using explicit scalar/source shims.
They comprise one positive constructor probe and four intended compile failures;
those shims do not establish real-crate representation or behavior. The separate
22-probe ordinary witness script above uses actual sibling crate consumers.

The final independent report SHA-256 is
`acb4c18227eea885a8dc4818d1451993a0cbee530705cc858630865f8ea6b29b`;
the exact comparison receipt SHA-256 is
`cc4439ba84652a3522061c69d977e41f666b5ad189c77e3327f0374cd7a212af`.
The reviewer preserved a failed positive-control fixture and a stale-target run
as unsuccessful/invalid evidence. Neither contributes to the pass; the qualified
final pair uses the explicit executable/source identity checks described above.

## Remaining boundaries

No source binding/pin refresh, new raw opcode, bounds operation,
reference/native array execution, native coordinate amendment, native array
pilot, new host qualification or hosted exact-head CI is claimed here.
Frozen source-transition and instrumentation authorities remain unchanged; later
binding integration must separately review their compatibility with migrated
carrier fields, including lifecycle/mutation overlays and the source-test adapter.
