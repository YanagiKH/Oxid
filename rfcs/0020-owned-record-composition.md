# RFC 0020: bounded owned record composition

Status: declaration-facade checkpoint; executable source and raw composition are
closed until independent verification and both consumers are complete.

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
