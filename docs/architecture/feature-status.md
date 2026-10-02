# Feature status inventory

[feature-status.json](../feature-status.json) is a non-exhaustive M0 inventory.
It records a source revision, bounded feature scope, implementation/test paths,
owner/reviewer assignments, and known limitations. `unassigned` is explicit and
must be resolved before a validation/stability claim; no maintainer commitment
is implied by adding an entry.

| Status | Meaning |
| --- | --- |
| `proposed` | Direction or acceptance target; not available functionality |
| `experimental` | Some runnable code exists, with incomplete contract or evidence |
| `implemented` | The scoped feature is connected to the production path and has tests; required validation may remain |
| `validated` | The declared scope/target matrix has reviewed execution evidence and an owner |
| `stable` | Validated with accepted compatibility/migration policy and review |

Classification is separate: `production-path`, `demo`, `synthetic`, or `planned`.
A demonstration can be tested without becoming a production feature. A feature
can have a passing local test without becoming validated on every target.

Every entry describes edition, backend, and target scope independently. `legacy-0.9` now also explicitly selects the default legacy CLI route.
The [typed-preview pipeline](../../spec/typed-preview.md) is an opt-in, experimental
frontend; it does not change the default edition. Test paths identify checks; evidence paths identify recorded results and
limitations. An empty evidence list means no report is attached to that claim.

Run `python3 scripts/verify_feature_status.py` to detect malformed status values,
duplicate IDs, missing/escaping repository paths, or claims missing required
records. It is also run by `scripts/verify_repo.py`. The validator checks metadata
consistency, not the truth of a safety, compatibility, performance, or release
certification. Review must examine the linked implementation and evidence.

The bounded bool/unit reference runner is tracked separately from its check-only
predecessors. It enables explicit typed run but does not certify termination,
ownership, OS isolation, native compilation or any completed milestone.

Exact decimal i32 literals and scalar copies are a separate experimental entry.
That predecessor does not implement arithmetic, casts, wider integers or floating
point. Checked i32 addition/subtraction/multiplication are a separate experimental
entry, with runtime overflow errors identical in debug and release. Other numeric
operations remain deferred.

The optional LLVM native preview is separately tracked as experimental. It accepts
a stricter bounded nonrecursive scalar subset, emits Linux x86_64 PIE executables,
and rejects unsupported operations before tool invocation. Its checked i32
`+`, `-` and `*` extension preserves reference overflow and first-error behavior;
other arithmetic operations and recursive native calls remain unavailable. It does
not complete M2/M3 or certify production safety, a stable ABI, or self-hosting.

Scalar comparisons are a separate experimental extension: same-type i32 or bool
equality/inequality, signed i32 ordering, bool results and no coercion. All six
operators share a non-associative tier below arithmetic. The checker, independent
OIR verifier, bounded reference executor and narrower LLVM backend enforce the
same table; unit equality and bool ordering remain unavailable. [Comparison evidence](scalar-comparison-validation.md) records the
actual supported checks and hosts.

Boolean !, && and || form a further experimental slice with bool-only operands,
full RHS static checking and runtime short-circuiting. Explicit two-input bool
joins preserve global single assignment and predecessor-edge availability; the
LLVM backend uses real branch/phi control flow. Full-file native structural checks include skipped RHS work; acyclic cost bounds
remain conservative, while guarded runtime fuel charges only executed paths. See [boolean logic evidence](boolean-logic-validation.md).

Initialized mutable bool/i32/unit locals are a further experimental extension.
Explicit typed places and init/store/load operations preserve immutable SSA value
snapshots; independent verification requires initialization dominance. Existing
if branches may update places, and native lowering uses private scalar allocas.
All scalar slot/fuel ceilings include those storage operations. That predecessor
does not add borrows or non-Copy ownership; the experimental ownership-foundations extension
is tracked below. Native recursion remains unavailable. See
[mutable-local evidence](mutable-locals-validation.md).

Ordinary bool-condition while is an experimental end-to-end extension with cyclic
OIR dominance and shared one-million-operation reference/native fuel. Guarded
native modules retain static call-depth/storage bounds and add explicit 16 MiB
human-diagnostic/64 MiB LLVM-text ceilings. Existing acyclic native admission is
preserved; no native recursion or final-performance claim follows.
See [RFC 0012](../../rfcs/0012-while-runtime-fuel.md) and
[while validation](while-validation.md).

Unlabeled break/continue extend the while subset end to end. Four-outcome typed
flow summaries separate return from loop transfers; lexical targets lower to
existing charged Goto edges without unreachable joins. No ownership or new type
is implied. See [RFC 0013](../../rfcs/0013-loop-control.md) and
[loop-control evidence](loop-control-validation.md).


Ownership foundations are one additional experimental capability, not separate
entries for declaration tables, raw verification, source parsing or consumers.
The extension covers nominal move-only structs with bool/i32/unit fields or no
fields, whole moves/replacement, scalar field access, owned helper returns and
explicit exact-mode call-only borrowing/reborrowing. The complete source module
selects one verified route; scalar-only modules retain their prior behavior and
costs. Default legacy records retain dynamic shared storage.

The entry is `experimental`/`production-path`: the public `typed-preview`
parser and driver select the ownership source route. Its
[source qualification ledger](owned-source-validation.md) records the exact
source and compiler identities and separates model-only, preactivation facade,
production CLI and native evidence. Native qualification is limited to Linux
x86_64 with LLVM 19.1.7 at O0; hosted checks apply only to the recorded exact head.
A gate's registration alone does not establish its result. The historical
[raw-consumer report](owned-consumers-validation.md) remains scoped to its
original fixtures and compiler identities.

The broad `static-memory-model` item remains proposed for unresolved language-wide
semantics. It must not be read as denying the bounded implemented verifier or as
claiming that this subset completes general ownership safety. The native item
cross-references the same ownership capability, without creating another feature
for it. Stored-reference lifetimes, partial moves, field-disjoint loans,
heap/resource cleanup, unsafe/FFI safety, broader native targets and stable ABI
remain outside this increment. No inventory count certifies a milestone or v1.0.
