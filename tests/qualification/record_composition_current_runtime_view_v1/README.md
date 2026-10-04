# Current scalar-record runtime observation view and transfer hook v1

This package is a corrective successor to `record_composition_current`, not a
change to its frozen predecessor or any compiler/expected-data source.

## Representation-only actual view

`runtime_view.project_bytes` accepts the canonical JSON bytes emitted by the
unchanged published Unit3 observer controller. It emits separate canonical JSON
and a per-site original/derived/round-trip identity receipt. `reverse_bytes`
restores original bytes exactly. Consumers must retain original observation
artifacts and receipts, bind this module hash, and verify each receipt before
comparison. The view has no file-writing or compiler-execution behavior.

Only these sites inside owned `raw` and `raw_before_audit` are transformed:

* `OwnerDecl.aggregate = AggregateSlot(Record(id))` becomes `record = id`
* `ReferenceDecl.referent = BorrowedSlot(Record(id))` becomes `record = id`
* `LoanDecl.referent = BorrowedSlot(Record(id))` becomes `record = id`
* Function result `Owned(Record(id))` becomes `Owned(id)`

Scalar results are unchanged. Exact tags, keys and tuple arity are checked.
Slot IDs must be actual JSON integers in u32 range; result RecordIds must be
integers in the qualified 64-bit usize range. Boolean-as-integer is rejected.
Deliberately invalid/out-of-range *declaration references* within those encodings
are preserved; the adapter never resolves them, drops them or substitutes an ID.
Every mapped site round-trips. Existing scalar-route and pre-raw rejection views
remain identity mappings, with route/root-tag consistency checked. Mapped owned
views cannot be adapted twice.

Non-scalar stored fields, arrays, slices, composite/array instructions and events
are rejected. No cost, result, diagnostic, span, instruction payload, source, audit,
trace or other non-raw subtree is rewritten. This view does not relax any frozen
comparator or expected result and provides no qualification verdict by itself.

Schema anchors and exact identities are recorded separately. The current Rust
carriers are in `src/frontend/oir/owned_types.rs`; descriptors are in
`src/frontend/oir/owned/mod.rs`. Current normalized JSON follows the unchanged
Unit3 `components/observer/parse_debug.py`. The old scalar-record JSON consumer is
`components/oracles/comparison-v1/actual_raw.py`, especially `value_type`,
`parameter`, `owner` and `frame_counts`.

## Real transfer write-site migration

`transfer_hook.derive` accepts only the exact prior derived `execute.rs` returned
by `record_composition_current/runtime_overlay.py`. Its one exact substitution
instruments the successful `transfer_payload` scalar-leaf write loop. It captures
actual destination bytes before the original `store_leaf`, preserves `?` failure
propagation and emits the original payload tuple only after success. Scalar root
fields are paired with actual leaves in order, checking exact offsets and types.
Empty-record sentinel writes generate no historical field rows. The original
Invoke parameter-copy exclusion is retained. No hook is installed across the
whole `store_leaf` helper, arrays, projections or incoming parameter encoding.
`transfer_hook.reverse` restores every prior-overlay byte; changes outside the
single loop are rejected by exact whole-file input/output identities.

`scalar-store-controls-v2.rs` retains the previous control source unchanged in its
original package. Its explicit current count is three initializer writes, three
actual MoveInitialize transfer writes, and three field assignments. The original
field-byte assertions and exhaustive lower-fuel no-unpaid-store check remain.

## Controls and qualification scope

Run both `test_runtime_view.py` (15 controls) and `test_transfer_hook.py` (4)
with `python3 -B`, then with `python3 -B -O`. Tests reject missing/extra keys,
wrong tags/arity, malformed IDs, route relabels, arrays/slices/composites and
repeated adaptation, while preserving representable invalid declaration IDs.

Append `transfer-controls.rs` to the corrected external execute module and
`transfer-journal-controls.rs` to the derived journal. The three real-machine
controls cover one actual write/no duplicate, failed scalar-store and failed
transfer-range non-effects, empty-record sentinel exclusion, and mixed
bool/unit/i32 width/order/padding preservation. Keep their exact source identities
in the external build receipt. They never alter ordinary compiler sources.

The first actual debug152 run is retained as failing evidence: all152 invocations
produced observations, but74 owned cases lacked transfer-field events. The new
view revealed that gap without concealing it. Fresh corrected debug/release
54-typing and152-runtime executions, immutable predicate comparisons, exact build
identities and independent review are required before reporting qualification.
No debug result can stand in for release. Historical compiler execution remains
`FROZEN_EXPECTATION_NOT_EXECUTED`; current-source evidence must be labeled current.
