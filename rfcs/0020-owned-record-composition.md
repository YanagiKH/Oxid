# RFC 0020: bounded owned record composition

Status: experimental implementation for independent review. The initial
published declaration-only checkpoint kept executable composition closed. The
current implementation extends the existing checked source and sealed raw
verification paths; exact-head hosted CI and the source-binding successor are
separate qualification requirements, not implied by local tests.

Baseline: published PR33 head `848b36bcb2e23a1560d6a3f7e84402818b78d3a4`,
tree `f8e2d39d8081b992969597649bad7fd9f28e7d04`. The local equivalent base commit
was `4c1c040556b216d5d044a029ed6f05825d446d76` with exactly that tree.
This increment advances bounded M2/M3 functionality; it does not complete
those milestones, Rust compatibility, memory-safety proof or v1.0.

Records may contain bool/i32/unit scalars, nominal records, and existing fixed
scalar arrays. Forward references are allowed; by-value containment must be
acyclic. Nominal identity and physical field order remain declaration-ordered.
Containment and named-root access paths have a maximum depth of 64. Layout is
computed bottom-up with checked padding and arithmetic, before output tables or
transitive leaf storage are allocated. Existing byte/count/work ceilings remain.
The expanded width of a scalar is one, an array is max(1, length), and a record
is max(1, sum of field widths). Empty storage retains existing positive sentinels.
Graph traversal uses bounded iterative scratch, never recursive host calls or an
exponentially flattened type tree.

A constructor evaluates complete initializer values once in written source order.
Each owned initializer is moved to staging before evaluating subsequent fields.
The destination becomes initialized only after all fields are present and the
whole-value work charge succeeds. Whole outer records remain move-only owners:
owned arguments/results, complete replacement and whole-root call borrows keep
their existing provenance, availability and conflict rules.

Named-root bounded field paths may end in a scalar read/write, a fixed-scalar-
array index or its len(). Every field hop must satisfy privacy and nominal type
checks. The raw verifier resolves paths independently; producer offsets are not
authority. Scalar leaf access keeps existing operation costs and evaluation
ordering; whole-value work uses recursive width and separately charged padded
bytes. Indexed writes keep RHS, index, fuel, bounds, store ordering.

Not supported: aggregate-field extraction or replacement, partial initialization
or moves, projected borrowing or slice conversion, nested arrays or arrays of
records, stored references, contextual empty-array-literal inference, arbitrary
dereference, grouped or temporary projection roots. These require separate work.

Acceptance pilot: Meta { completed: i32 }, Batch { meta: Meta, samples: [i32; 3] }.
Move complete children into Batch; relay Batch through owned parameter/result;
borrow its whole root exclusively to increment every sample and completed once
per element. Starting [1,2,3]/0 returns completed*100+samples[0]*10+samples[2] = 324
in the reference runner and source-free native artifact. Qualification also
covers privacy, modules, reborrows, initializer order and moves, malformed paths,
duplicate consumption, cycle/depth/width boundaries, and existing regressions.


## Representation and resource contract

Checked `FieldDecl` retains `ValueTy`. Nominal record IDs stay in declaration
order; a checked iterative containment traversal computes each record's padded
layout and recursive width bottom-up. Direct and mutual cycles reject, including
unused declarations. References never become stored values. The graph holds
O(records) scratch and a depth-64 stack, not an expanded tree.

The source-only value-layout inventory and final checked declaration facade use
the same bounded graph calculation. Both verify producer lengths, nominal IDs,
checked widths and padded sizes before executable allocation. At the qualified
64-bit representation, checked records/fields occupy 72/64 bytes; the original
8 MiB declaration table cap is retained. The sum of declared padded layouts is
still limited to 1 MiB. Existing runtime 200,000 expanded-cell/16 MiB storage and
native 8,192 expanded-cell/1 MiB arena limits are unchanged.

Composed constructors retain scalar operands or complete temporary owner IDs.
Each owned source expression stages through a whole-value move before lowering
later initializers. Independent raw shape checking verifies complete field
coverage, nominal types, staging class, and unique child consumption. Existing
whole-owner CFG availability and loans validate every consumption and projection.
No field path is a place owner, loan authority, reference value, or array owner.

A path carries nominal field IDs, never a producer-supplied offset. Verification
resolves every hop from the checked root type. Consumers rederive offsets using
the same immutable declaration facade. Lazy leaf traversal uses fixed depth-
bounded stack storage, includes empty-value sentinels and excludes padding.
Whole transfers, arguments and results use recursive width. Unchanged scalar-
only operations keep their existing fuel formulas and source ordering. New raw
initializer/path payloads contribute independently to metadata, event and work
admission; no existing ceiling is raised.

## Qualification boundaries

The raw pilot has an independently specified 269-fuel schedule; this is not the
source pilot's lowering cost. Every lower fuel budget must fail at the expected
source span, with no unpaid store. Source and raw evidence are separate. Existing
historical record/array/slice ledgers remain tied to their original compiler;
this RFC does not retroactively expand those qualification claims.

See the [local validation ledger](../docs/architecture/record-composition-validation.md)
for tested scope and explicit qualification limits.
