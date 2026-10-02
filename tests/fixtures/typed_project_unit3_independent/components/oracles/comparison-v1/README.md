# Independent actual comparison, review candidate v1

Original semantic expectations remain frozen at b89c8c5b…, with accepted
source-only declaration supplement8dd370bf… and native-stream supplement9cae46e8….
This directory contains separately versioned comparators and actual reports.
It does not modify any prior frozen source/model file.

## Main comparison pipeline

expected_projection.py reads and verifies the original and declaration manifests.
It projects only already-frozen IDs, body facts, schedules and events into
neutral expectations. It does not read candidate outputs or resolve source.

actual_raw.py consumes only actual canonical raw structs. It joins actual IDs to
their actual declaration/site spans and roles, reconstructs actual parameter
types from actual binding tables, identifies actual call/field/owner operands,
and counts actual frame tables. It imports no expectation/model/source parser.

compare_static.py checks two separate things: complete raw association and
source/raw correspondence. Every actual count-pass and validation-pass visit
must match the independently enumerated path multiset and stored span value.
Both pass counts and fixed dimension counts are checked. Source files are bound
by bytes, all declared fields use the accepted supplement, and wrapped entry
must be the frozen original-root ID. Static calls, nominal fields, owner roles,
source bindings, constructor field order, transfers and stores compare actual
raw facts with frozen source facts. A same-signature wrong target fails direct
numeric target comparison even if its raw verifier accepts.

actual_trace.py is expectation-free. It mechanically joins each actual runtime
context to the immutable actual raw operation at that function/block/position,
then retains every attempted charge, paid cost, denied cost and committed effect.
It checks that one fuel counter is never reset, an unpaid operation has no
effect, and a successful operation commits at most once. It does not synthesize
missing operations from the expected charge schedule.

Owner runtime generations are retained in raw_epoch_journal and checked to
increase by exactly one at each actual transition. Raw activation IDs come from
actual Enter events and receive chronological comparison ordinals. Source-style
lexical lifetime counters are separately derived from actual StorageLive/staging
initialization, actual new call-result availability and new owned parameter
transfers, with raw owner roles. Whole replacement keeps its lexical lifetime;
its runtime epoch still changes. No fixed generation offset and no expected
event lookup defines these actual keys.

Actual borrow acquisitions retain the exact runtime OwnerKey generation and
LoanKey instance. Forwarded-parent identity comes from actual caller argument
loan slots joined to actual callee reference-parameter bindings at invocation.
Release must name the exact active loan, occur in the actual returning callee,
and leave no active child behind. The resulting acquire/release stream is
compared separately with frozen source loan events.

Physical scalar-place writes, field payload bytes, and grouped whole-value
replacement writes are compared with frozen committed source writes at each
budget. Mutable scalar initialization is included using the already-frozen
let-scalar events. SSA expression evaluation is represented by the full charged
operation stream; it is not mislabeled as a mutable source store.

compare.py combines strict byte/diagnostic checks, static correspondence and
complete attempted operation/target/owner/fuel/origin prefixes. An omitted or
reordered StorageEnd is detected from the actual operation sequence and actual
owner operand, without synthesizing an expected cleanup event in the observer.
Unknown trace variants and unresolved actual sites fail closed.

The v2 review corrections make actual callback rosters exact per raw operation:
each transfer, transition and payload effect must name the raw-required source
and destination, actual activation/function/frame and correct pre-/post-epoch.
Effects after commit are rejected. Full source-model transfer identity streams
are compared too, including owned parameter installation and ReturnOwned into
the caller result slot. Successful completion requires an empty actual call
stack, including the root return callback. Loaded file IDs/order/bytes/hashes
and request-bound display paths are checked before the raw conditional, so
early source rejections receive the same source identity validation.

Physical i32 fields require exactly four little-endian bytes; bool/unit require
one byte, bool0/1 and unit0. Recorded Scalar type must match the raw field type.
Standalone primitive controls cover valid i32 endpoints, bool, unit and nine
malformed encodings. Nine independently authored synthetic stream corruptions
and26 altered actual hand-smoke journals exercise dropped/duplicate callbacks,
post-commit effects and malformed move/physical-target/transition identities.
The actual journal controls infer no model outcomes and retain original bytes.

The final v3 correction also keeps LoanKey.frame in full runtime identity and
checks Acquire/Release frame ownership through each key's actual activation.
Acquired OwnerKey is compared in full with the current owner key, including its
owner activation frame, which can differ from the current callee frame. Loan
instances must increase once per actual acquisition. The three reviewer frame
counterexamples are retained and rejected, bringing actual journal mutants to29.
Earlier v1/v2 snapshots and their sensitivity reports remain in history/.

All executable comparator/test modules reject Python optimization. Invocations
explicitly set PYTHONOPTIMIZE=0; assertion mode is part of validation receipts.

The105 model-backed cases have detailed static/runtime comparison. Authored
cases use exact declaration association and their explicitly frozen diagnostic,
entry/result or native contract; they are not relabeled as full tagged-body
model coverage. Runtime epoch checks still apply when a reference trace exists.
The corpus gate checks exact case/profile inventory unless --allow-subset is
explicitly requested for a reviewed limited run.

## Native and driver streams

compare_native.py verifies the native supplement, exact source identities,
driver stdout/stderr/status and source-free executable stdout/stderr/status.
The four admitted regular/symlink controls each run text/JSON; two recursion
controls are separate early-admission JSON cases. Runtime E0604 in a collision
descriptor is an executable failure after successful compilation, as explicitly
frozen in the native descriptor; it is not an admission error.

Build/tool/source-unavailability/ELF provenance is independently audited by the
core reviewer and qualifier. This comparator's reports deliberately name their
semantic-stream scope instead of treating bytes alone as execution provenance.

## Validation and correction history

Before the full modeled corpus runs, both-profile hand-smoke observations were
used only to test mechanical transport/joins, with no semantic expectation
derived from their outputs. Corrections fixed transport assumptions: generic
Event Scalar payloads use tuple items; scalar_place_write carries the actual
place-use span rather than its declaration span; LoanKey uses instance, while
OwnerKey uses generation. Those corrections did not edit frozen expectations.

Native smoke/full semantic comparison retained a runtime-versus-admission
dispatch error and correction separately in history/. The original comparator
version is reconstructed byte-for-byte to its previously reported SHA and
labeled as such. Candidate receipts and original failures are not overwritten.

## Reviewed same-return release projection (v5)

The first full debug comparison, preserved in full-debug-incremental-attempt1.json,
matched 151 cases and rejected positive-exclusive_alias because the independent
model iterates sibling releases in reverse order. RFC0014 section 4 requires
loans to end at the owning direct call's normal return; Unit3 section 5 requires
reverse lexical StorageEnd cleanup, without specifying sibling release order.
The frozen predecessor consumer releases the same set in declaration order.
The unchanged model and all original physical actual journals remain retained.

The source-contract rationale is proposals/same-return-release-contract.md.
The v5 projection compares membership only inside an exact charged normal
Return, identified by returning activation and its Enter-time resume call.
Every actual release retains its complete runtime key and physical order.
The normalizer validates exact caller activation, function, CallSiteId, and
borrowed LoanKey membership/instances before producing a source-key projection.
Missing, duplicate, shifted, wrong-owner, and parent-before-child releases fail.
Acquisition, group, charge, effect, transfer and StorageEnd order remain strict.

The retained control script test_return_release_groups.py checks one permitted
sibling permutation and six invalid transport/group mutations, including two
returns from the same static call site with distinct actual loan instances.
This is a comparator contract correction. It changes no expected source result,
fuel boundary, frozen source/model event, compiler code, or actual receipt.
