# Record-composition current observer adapters v1

This is a bounded set of pure, identity-checked byte derivations, not a replay
framework or a new language oracle. Frozen Unit3 files, expectations, controllers,
portable assemblers and original identities remain unchanged. Nothing writes to
compiler source or accepts a different original implicitly.

## Integration entry points

Import this directory explicitly; the modules are not automatically loaded by
historical or current qualification runners.

* `observer_adapters.derive_typing_observer(original)` accepts the exact archived
  `fixed_array_source_unit3/typing-replay-v1/components/observer.rs` and returns
  the current-compatible observer. Eight exact substitutions preserve scalar
  field JSON, borrow non-Copy projections, require exactly `[field]` paths, map
  borrowed `Exact` types, reject slices/aggregate fields, and explicitly convert
  the current trace-reservation error type. Reverse function restores every byte.
* `current_hooks.derive_typing_overlay(originals, original_observer)` accepts
  exactly `TYPING_PATHS` from the current compiler and returns four test-hook
  files plus `src/frontend/oir/owned/source/array_typing_observer.rs`. Hooks keep
  the historical lex-attempt/success and parse-attempt/success order and phase
  accessor. Existing current trace allocation/limits remain authoritative.
* `runtime_overlay.derive_runtime_overlay(originals, original_prepare,
  original_patch)` accepts the eleven original files in `CURRENT_FILES`, the
  frozen observer `prepare.py`, and frozen `overlay-v5.patch`. It returns eleven
  instrumented files plus three observer modules. Only pinned archived helper
  definitions and source transformations are evaluated; its CLI, filesystem,
  baseline manifest and build code are not run. Exact output identities are
  checked as well as exact input identities.
* Individual reversible functions cover the exact scalar `store_field` body,
  archived preparation recipe, raw owned registry and observer journal. The
  current `record_event` helper has a separate reversible current-hooks adapter.

Materialize each returned mapping into a fresh external copy whose source is
already bound to the current source manifest. Never apply these derived patches
on top of an already adapted tree. The runtime and typing mappings have disjoint
paths and may be combined. Keep the selected source manifest, original package
identities, each derived mapping, and the actual current build identity together
in the new current-only receipt. A current build must not reuse the archived
qualified-binary identity or imply historical package membership.

## Historical-domain boundary

The raw registry rejects all non-scalar stored fields, array owners/results,
array/slice references and loans, and every new array/composition instruction
before capture or registration. Its instruction match is exhaustive. New enum
variants will require an explicit adapter version, rather than a wildcard.
When its journal is disabled, the raw registry returns without imposing this
historical qualification domain on ordinary compiler operation.

Scalar-store observation captures before bytes only after original field/type
checks. It calls the same `encode` and propagates its error before emitting
`payload_write`. The payload tuple is unchanged: key, field, value, span,
optional before bytes, optional after bytes. Existing owned events now live in
`array_observe.rs::record_event`; the exact helper adapter emits after each of its
two successful pushes and never on the truncation/rejected-allocation branch.
Without this migration the old preparation regex silently observes zero events.
No array/projection storage helper is swept into the scalar-store migration.

## Existing producer entry points

The frozen typing `replay.py` archives its old Git head and accepts only its old
qualified binary for reuse. It must not be retargeted. The smallest actual
producer entry point in the derived current test binary is:

`frontend::oir::owned::source::array_typing_observer::array_typing_observe_requests`

Run it with `--exact --ignored --nocapture --test-threads=1`, and set
`OXID_ARRAY_TYPING_REQUEST` to the eight-column TSV and
`OXID_ARRAY_TYPING_OUTPUT` to a fresh external directory. Existing immutable
`typing-contracts-v1/source-manifest.json` supplies the 54-case roster. The
existing replay source shows request construction and comparison. Preparation
here does not claim those 54 cases passed and does not change any comparison.

The runtime producer remains `frontend::unit3_observer::observe_request`, called
by the unchanged Unit3 `components/observer/run.py`. The portable assembler and
native wrappers intentionally support only old core-v1. A current wrapper must
bind its current build/derived overlay explicitly rather than forge their
historical core manifest. The frozen native `overlay-v2.patch` needs only exact
context/current-identity rebinding; inspected fuel, module and tool-hook anchors
remain present. No native hook logic is changed by this package.

## Controls and evidence

Run `python3 -B tests/qualification/record_composition_current/test_observer_adapters.py`.
Run it again with `python3 -B -O`. Both run 13 controls: exact identities,
mutations, double adaptation, reversals, exact patch apply/reverse, explicit
membership, historical failed store reproduction, and confined store/event seams.

For Rust controls, append `raw-walker-controls.rs` to the derived raw owned
module, `runtime-controls.rs` to the derived journal, and `typing-controls.rs` to
the derived typing observer. Run `cargo test --locked --offline -j1 --bin oxid
record_adapter_ -- --test-threads=1` on that external tree; set
`OXID_RECORD_OBSERVER_CONTROLS` to an external evidence directory. The nine
controls use real current types and cover scalar JSON, owner/shared/exclusive
single-hop projection JSON, empty/multihop/mismatched paths, slices, aggregate
fields, every excluded instruction family, passive disabled operation, all six
bool/i32/unit initializer/write events and their ordering, and every lower fuel
budget before a simple scalar-store program first succeeds.

Fresh implementation verification compiled the complete migrated runtime and
full typing observer together. Nine equivalent real-type controls passed; typing
controls were instantiated from the exact adapted helper/display definitions in
a sibling test module. No 54-case semantic/resource replay or release producer
qualification was performed. Historical representation/resource expectations are
not portable by assumption: current field/projection/path/frame sizes differ.
These need an independently calculated explicit current amendment, never values
copied from candidate output. Debug schema changes elsewhere are not normalized
or rewritten by these adapters.
