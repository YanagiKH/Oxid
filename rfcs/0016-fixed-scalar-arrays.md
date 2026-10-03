# RFC 0016: fixed scalar arrays and checked indexing

Status: proposed language capability; Unit 1 groundwork and gated Unit 2A–2B
private identity carriers and raw verification.
Array syntax and executable raw array operations are not enabled by this RFC or
the current preparatory slices. The current public contract remains
[RFC 0014](0014-owned-structs-call-borrows.md) and
[RFC 0015](0015-bounded-typed-projects.md). Implementation ownership and independent
review are recorded by the associated pull request. Acceptance of groundwork
does not accept or qualify later source/native activation.

Baseline: commit `0ef3be1df3643febdff1f859a4eb1ce567ab8164`, source tree
`24bfdb84b042899f655e5dab472d2ad8be24c762`. This is one bounded collection
capability toward M2, not M2 completion, Rust compatibility, a heap collection,
production CLI, CPU/GPU tensor support, or native AI training.

## 1. Outcome and preserved boundaries

A typed program will construct a complete fixed scalar array, move it through
helper parameters/results, borrow the complete array during a call, and process
it with checked signed indexing and a narrow length intrinsic. It works in an
original single file or a bounded linked project. Arrays of the same element
type and length agree across modules; records retain their nominal identity,
field privacy, layout and costs. Executable root `main` remains zero-argument
and returns bool/i32/unit. `check` may eventually accept an array-returning main;
`run` must reject it with E0600 and native compile with E0700 before tool/output
work. There is no array printing or native ABI exposed to users.

Only explicit `--edition typed-preview` routes into this frontend. Neither
brackets nor annotations select an edition by content. Default and explicit
legacy-0.9 commands, dynamic arrays, shared legacy mutation and OXBC remain
unchanged. Array-free typed programs retain admission, costs and diagnostics.
Refinements of formerly unsupported array syntax are new behavior only when
Unit 4 explicitly activates it.

Excluded: nested arrays, arrays in record fields, arrays of records/references,
element moves/borrows, destructuring, partial initialization, slices/ranges,
dynamic lengths, repeated-element syntax, implicit copying/cloning/equality,
iterators/for loops, heap allocation, destructors, stored/local/returned
references, generics, const expressions, new numeric widths, arbitrary methods,
temporary/grouped indexing bases, general dereference, FFI, and new targets.
By-value parameters keep their current immutable-binding rule; moving into a
mutable local or passing an exclusive borrow enables mutation.

## 2. Proposed grammar and inference

```text
scalar_type     := "bool" | "i32" | "(" ")"
array_type      := "[" scalar_type ";" decimal_length "]"
decimal_length  := ASCII_DIGIT+
value_type      := existing_value_type | array_type
parameter_type  := existing_parameter_type | "&" array_type | "&" "mut" array_type
array_literal   := "[" (expression ("," expression)* ","?)? "]"
array_read      := local_name "[" expression "]"
array_write     := local_name "[" expression "]" "=" expression ";"
array_length    := local_name "." "len" "(" ")"
```

The scalar element is exactly bool/i32/unit; the nonnegative decimal length is
inclusively 0..1024. Leading zeros are allowed (`[i32; 0008]` equals `[i32; 8]`).
Signs, separators, expressions and named constants are excluded. Parse decimal
digits with bounded checked arithmetic before allocation; reject a value above
1024 as E0400/parse at the entire length token. Token/source limits still run
first. A very long all-zero length is governed by the existing token cap,
without numeric overflow. Missing required punctuation is E0100/parse;
recognized excluded forms are E0101/parse.

A nonempty literal infers its scalar element from the first element, then
requires every remaining element to match exactly. Length is its exact element
count, bounded separately by 1024; it does not inherit the call-argument cap
256. Elements evaluate once left-to-right, including unit-valued effects. The
array becomes available only after all elements succeed. No partially
initialized array is observable.

An empty literal is accepted only as the initializer of an explicitly annotated
local of length zero, with parentheses transparent around that literal:
`let a: [i32; 0] = ([]);`. This is a narrow initializer typing entry point, not
general bidirectional inference. `return []`, `take([])`, `a = []`, an
unannotated `let a = []`, and a nested empty literal are rejected even when some
surrounding signature could supply a type. Use a typed local and move it instead.
An explicit nonzero annotation on `[]` is a length mismatch.

Index/length bases are exactly a named owned local/parameter or a named borrowed
array parameter (`a[i]`, `p[i]`, `p.len()`). `(a)[i]`, `make()[i]`, `[1][0]`,
`(*p)[i]`, `(a).len()`, `a.len(1)` and `a.other()` are excluded. Grouping the
whole expression or the index is allowed. The spelling `len` remains an
ordinary name and record field; `record.len` keeps field semantics. `a.len()`
is only this array intrinsic, not global name lookup or method dispatch.
The parser recognizes this exact zero-argument spelling on a named base; typing
then rejects a non-array base with E0305. Thus `record.len` is unchanged while
formerly unsupported `record.len()` may acquire that refined diagnostic after
activation. No such diagnostic change occurs in Unit 1.

## 3. Types, layout and ownership

Array identity is the ordered pair `(scalar element, length)`. Equal layouts
do not imply equal types: `[bool; 4]`, `[(); 4]`, `[i32; 1]` and a four-byte
record are distinct. `[bool; 0]` and `[i32; 0]` remain distinct. Nominal records
never become arrays through their numerical RecordId. Structural types contain
no source span; use-site origins retain their own file ID.

Private native element size/alignment is bool 1/1, unit 1/1, i32 4/4. Stride
equals element size. Array size is
`align_up(max(1, checked(N * stride)), alignment)`. Thus `[bool; 0]` and
`[(); 0]` use one byte, `[i32; 0]` uses four bytes, and `[i32; 1024]` uses 4096
bytes. These are positive private storage identities, not a public/C ABI or
Rust layout claim. A zero array's reserved bytes are initialized to zero and
never readable as elements. Unit elements have canonical initialized storage.
Construction/transfer initializes reserved sentinel/padding bytes; array
transfers preserve the logical sequence and never read uninitialized padding.
Record field-wise transfers keep their existing padding policy.

Every array, including zero/unit arrays, is move-only. Binding, passing by
value, returning, discarding and replacement transfer the complete owner.
Whole replacement/self-move follows RFC 0014. Indexed reads copy a scalar
snapshot. Indexed writes require an available mutable owner or an exclusive
reference; writing an element cannot reinitialize a moved array. Availability
includes compiler temporaries/results and all branch/loop/continue edges.

Borrow parameters are `&[T; N]` and `&mut [T; N]`. Existing complete-argument
forms `&a`, `&mut a`, `&*p` and `&mut *p` apply, with exact modes, immediate
left-to-right acquisition, whole-owner overlap, parent/child permissions and
normal-call-return release. No element-disjoint loan analysis is introduced.
Staged argument owners are not general read/write bases. Length is always a
verified whole-base Read, even though N is static: `consume(a); a.len()` and
`f(&mut a, a.len())` must fail availability/loan checks.

## 4. Indexed effects, diagnostics and fuel

An index is exactly signed i32. Every access checks `0 <= i < N`, including
literal indexes and unit elements. Negative indexes do not wrap; there is no
clamping/conversion. Every index into a zero array fails. Literal out-of-bounds
access is a runtime failure in an executed path, never an eager failure from
an untaken branch.

For `a[index] = rhs`, first resolve the target binding, then resolve/number the
complete RHS expression tree, then the complete index expression tree. Parsing
may retain lexical order. Completed HIR expression IDs and lowering count/emit
walks must follow RHS then index, preserving the existing completion invariant.
For reads, resolve the target binding then its index. For writes, type RHS
first, then index; after their internal errors, check array base kind, exact
i32 index, element/RHS type, and binding mutability, in that order. Ownership
and loan checks follow valid lowering. These new rules do not reorder existing
record/scalar diagnostic paths.

Runtime order is distinct from static diagnostic order:

| Operation or failure | Required effect/order |
| --- | --- |
| Literal | Evaluate every element once left-to-right, then charged full construction |
| Read | Evaluate index once, pay one access fuel, check bounds, then load scalar |
| Write | Evaluate complete RHS to scalar snapshot, evaluate index once, pay one access fuel, check bounds, then final store |
| Length | Statically require a readable available base; at runtime pay one fuel before consumer validation and producing i32 N |
| RHS failure | Earlier effects survive; skip index and final access |
| Index failure | Earlier RHS/index effects survive; skip final access charge/store |
| Insufficient access fuel | E0601 wins over bounds; no final read/write |
| Paid access, invalid bounds | E0606 wins before address formation/read/write |
| Successful final store | Install the previously computed RHS snapshot once |

Temporary call loans created while evaluating RHS/index end on that call's
normal return, before final access. An index helper may borrow/mutate a and
return 0. Moving a while computing an index instead makes final access fail
ownership verification. If RHS snapshots old `a[0]` and index evaluation changes
`a[0]`, final store writes that old snapshot. A failing final store performs no
final indexed write; mutations already performed by RHS/index, including to
the same array, remain. This is not rollback or a reserved LHS/two-phase loan.

| Rejected form | Code / stage / primary origin |
| --- | --- |
| Excluded grammar, nested/reference element type syntax, repeat syntax | E0101 / parse / unsupported construct |
| Missing punctuation/expression | E0100 / parse / current required token |
| Length or element count above 1024 | E0400 / parse / length token or first excess element |
| Unknown base/local/callee | E0200 / resolve / name |
| Empty literal outside the narrow context | E0300 / type / literal |
| Heterogeneous/non-scalar element | E0300 / type / first invalid element |
| Literal/annotation or call/return identity mismatch | E0300 / type / mismatched value |
| Indexing/length on a non-array base | E0305 / type / full access |
| Non-i32 index | E0300 / type / index expression |
| Indexed RHS element mismatch | E0300 / type / RHS expression |
| Immutable owner write/exclusive borrow | E0304 / type / target/borrow |
| Unavailable array, including length/store | E0310 / ownership / full access or rejected transfer |
| Conflicting active loan | E0311 / ownership / access or borrow |
| Bare reference value | E0312 / type / reference use |
| Shared reference write/exclusive reborrow | E0313 / ownership / target/borrow |
| Invalid raw IDs/types/origins/operations | E0500 / relevant OIR stage / validated origin only |
| Index out of bounds | E0606 / oir-owned-run / full `a[index]` target or read |

E0606 text is exactly `array index out of bounds`. No runtime numeric formatting
or index value is added. Native embeds the same bounded source-rendered message
and exits 1, including acyclic programs and source-free binaries. Existing
overflow/fuel messages remain byte-for-byte unchanged. Primary access spans
exclude assignment RHS and semicolon. Length origin is the full `a.len()`.
Secondary causal ownership spans follow RFC 0014. New diagnostic texts remain
under its retained-message limits.

Array width is `w = max(1,N)`. Substitute this width wherever existing consumer
costs use an owner's field width, including every local, temporary, staged
argument, parameter/result, frame initialization, transfer, replacement, return
and storage teardown. Literal operands retain their own evaluation costs;
construction costs `1+w`, transfer `1+w`, replacement `1+2w` under current
operation rules. Indexed read/write and length cost one after their operands.
Source fuel must be derived from actual lowering, not copied from a raw test.

## 5. Integrated three-module pilot

These are proposed fixtures, not currently accepted source.

`main.ox`:

```text
mod samples;
mod stats;
fn main() -> i32 {
    let original = [5, -2, 7, 0, -1, 9, 4, -3];
    let mut values = crate::samples::relay(original);
    let mut result = crate::stats::create();
    crate::samples::dispatch(&mut values, &mut result);
    return crate::stats::count(&result) * 1000
        + crate::stats::sum(&result) * 10
        + crate::samples::checksum(&values);
}
```

`stats.ox`:

```text
pub struct Stats { count: i32, sum: i32 }
pub fn create() -> Stats { return Stats { count: 0, sum: 0 }; }
pub fn record(s: &mut Stats, value: i32) -> () {
    s.count = s.count + 1;
    s.sum = s.sum + value;
    return;
}
pub fn count(s: &Stats) -> i32 { return s.count; }
pub fn sum(s: &Stats) -> i32 { return s.sum; }
```

`samples.ox`:

```text
pub fn relay(a: [i32; 8]) -> [i32; 8] { return a; }
fn filter(a: &mut [i32; 8], s: &mut crate::stats::Stats) -> () {
    let mut read = 0;
    let mut write = 0;
    while read < a.len() {
        let value = a[read];
        read = read + 1;
        if value < 0 { continue; }
        a[write] = value;
        write = write + 1;
        crate::stats::record(&mut *s, value);
    }
    while write < a.len() {
        a[write] = 0;
        write = write + 1;
    }
    return;
}
pub fn dispatch(a: &mut [i32; 8], s: &mut crate::stats::Stats) -> () {
    filter(&mut *a, &mut *s);
    return;
}
pub fn checksum(a: &[i32; 8]) -> i32 {
    let mut i = 0;
    let mut total = 0;
    while i < a.len() {
        total = total + (i + 1) * a[i];
        i = i + 1;
    }
    return total;
}
```

Independent sequence filtering gives `[5,7,0,9,4,0,0,0]`, count 5, sum 25,
weighted checksum `5+14+0+36+20=75`, and `5000+250+75 = 5325`. Retain the
complete sequence/effect/owner/loan trace: the final number alone cannot prove
correct stores. Require public check/run/compile and actual source-free ELF
execution; negative/upper indexes, moves, aliasing and effectful RHS/index
variants must retain precise failures and earlier effects.

## 6. Four implementation and review units

1. Contract and checked aggregate identity/layout. Add `FixedArrayTy`, explicitly
   tagged `AggregateTy`, and checked declaration facade queries. Use the common
   query in production record validation/planning. Retain record-only executable
   carriers and source grammar; no array raw witness can be built. Freeze this
   RFC and measure representation/admission compatibility independently.
2. Raw semantics and consumers. Migrate values/owners/references/loans together
   to aggregate identity, add constructors/index/length operations and complete
   shape/CFG/availability/loan/owner-class coverage. Recheck identity on reference
   incoming arguments/results/transfers. Add checked reference/native execution,
   bounds diagnostics, storage/transfer and complete resource accounting. Public
   source activation stays closed.
3. Source integration. Add AST/type/resolution/lowering, typed empty initializer,
   RHS-first two-root completion, nested-vector inventory and file-aware origins.
   Update both original-file `ast::uses_owned_syntax` routing and the independent
   metered project selector. Array-only and unused-module array syntax selects
   one aggregate route for the complete loaded project; no per-file fallback.
4. Independent qualification and activation. Extend the existing oracles and
   pilot, freeze/review actual production bytes, qualify applicable host gates
   and activate one bounded array capability. Publish actual new case counts and
   limits, without treating old record/project totals as new array evidence.

For Unit 2 every new operation must be enumerated in every all-operation shape,
scalar definition/use/dominance, availability, loan access/permission and owner
class pass, including unreachable malformed operations. Constructors use all
their scalar operands; read defines a scalar and uses index; write uses index
and RHS; length defines i32 and always reads its owner. A standalone bounds
claim or unchecked pointer instruction cannot authorize array access.

Native first pays access fuel where guarded, then tests signed `0 <= i < N`.
Only its success continuation converts/multiplies the index and forms/uses an
element pointer from checked layout. No new `inbounds`/`noalias` promise is
implied. Maximum local element offset is 4092, but cumulative owner offsets
still need checked preflight. Track each emitted continuation label, including
mixed arithmetic/bounds checks; acyclic short-circuit phis must name the actual
final success predecessor. Guarded and acyclic cases need separate evidence.

## 7. Unit 1 API and representation decision

`RecordId(usize)` and field-qualified `FieldId` stay unchanged. The internal
`FixedArrayTy::check(hir::Ty, usize)` constructor checks length before narrowing
to a private u16. `hir::Ty` admits only bool/i32/unit. Its fields are private;
element/length/stride accessors do not grant ownership or execution authority.
`AggregateTy` has distinct Record and FixedArray variants, without bit tagging,
reinterpretation, nominal array declarations or an interning table.

`Declarations::check_aggregate_type`, `aggregate_layout`, `aggregate_width` and
`same_aggregate_type` check nominal RecordIds against the actual table. Equality
validates actual then expected, before comparison: even two equal malformed
RecordIds reject. Array layout derives from checked element/length, never raw
supplied sizes/strides. Existing value/parameter type validation and execution
plan record width/layout use these shared queries. Existing record raw/storage
carriers remain RecordId in Unit 1, a deliberate gate until all Unit 2 passes
can migrate together. This is a production-used representation seam, not array
execution or an alternative witness.

The new descriptors are transient values: no persistent table, interning lookup,
new heap allocation, AST form, raw array operand vector or per-occurrence field
expansion is introduced in Unit 1. Length validation/layout/width are O(1).
Zero length never permits invalid element types or malformed nominal IDs.

### 7.1 Gated Unit 2A retained-carrier amendment

The [Unit 2A ledger](../docs/architecture/fixed-array-unit2a-validation.md) records
an explicit amendment to Unit 1's transient-only representation. Semantic
`ValueTy::Owned` and reference `ParameterTy` now carry `AggregateTy`; retained
owner/reference/loan and diagnostic-subject rows store an eight-byte
`AggregateSlot` with an independent record/array tag. Its checked conversion
stores nominal ordinals as u32 without changing `RecordId(usize)` or treating
an invalid nominal ID as an array. Declaration queries still validate semantic
identity before equality, with each raw verification site's old failure mapping.

The only raw construction exception is a retained RecordId above u32::MAX:
conversion fails immediately with the original `InvalidRecordId`. Such values
cannot identify valid records under the unchanged 4096-record cap. Full-width
semantic descriptors and raw declaration/field IDs are unchanged. A representable
invalid ordinal preserves resource-before-validation tests; separate controls
cover the earlier conversion failure.

Unit 2A explicitly rejects fixed-array carriers at authoritative raw admission,
after existing resource/declaration checks and before any executable witness.
The gate covers every result, owner, reference and loan, including unused and
infinite-loop cases. The declaration query facade may still describe arrays.
No opcode, source grammar, routing selector or array consumer is enabled. The
gate remains until subsequent Unit 2 verifier and consumer work is complete.

### 7.2 Gated Unit 2B raw-verifier slice

The [Unit 2B ledger](../docs/architecture/fixed-array-unit2b-validation.md) records
four private raw operations: `ConstructArray`, `ReadIndex`, `WriteIndex` and
`ArrayLength`. Their constructor operands, scalar definitions/uses, whole-owner
availability, permissions, loan accesses and denial origins use the existing
authoritative shape/CFG/flow pipeline. The cfg(test)-only probe borrows raw input
and returns only usage or failure; it cannot produce an executable witness.

Production admission still rejects every array carrier and now every array
opcode before seal construction, including inactive and malformed opcode-only
inputs. Reference dispatch rejects unsupported execution, while infallible
consumer matches have explicit sealed-invariant assertions. These temporary
arms must be replaced before the gate is removed. This slice does not implement
array reference/native execution, bounds diagnostics or source syntax/routing.

Raw preflight independently counts actual constructor operands as Q and rejects
a vector above 1024 before inspecting its elements. Q contributes to expanded
events, verification work and the exact Operand payload in ownership/source
metadata; no per-element verifier scratch is introduced. Source lowering still
emits Q=0. FunctionCounts gains one transient usize, while retained raw, AST and
runtime row envelopes remain unchanged. Source association separately walks all
new operand spans without providing source grammar or lowering authority.

## 8. Resource and compatibility obligations

Preserve all existing count and byte caps, including source 1 MiB, tokens/nodes
100,000, expression nesting/active statement blocks 64, arguments per call and
parameters per function 256, linked modules 256 with module depth 32, records
4096, fields per record 1024,
fields 65,536, declaration payload/layout 8/1 MiB, raw source payload 64 MiB,
ownership work 100M, ownership metadata/flow scratch 32/32 MiB, and consumer
admission. No per-file reset or hidden increase in existing counts is allowed.

Unit 1 must measure both new descriptors and the enclosing unchanged AST,
ValueTy, ParameterTy, declaration, raw ownership and runtime representations.
Its lack of retained representation changes preserves the existing measured
`R*sizeof(RecordDecl)+F*sizeof(FieldDecl)` payload and 288N AST envelope. This
must be shown by tests and explicit size measurements, not assumed from the
array descriptor's size. The 288N envelope and old 32 MiB ledgers are not a
permission to widen carriers in Units 2/3 without remeasurement and review.

Future array literal nodes/elements, constructor vectors, raw/source maps,
selector visits, origin walks, call staging/results, plan storage and native
emission each need exact inventories. Count actual expanded N-element work
despite compact metadata. Include every simultaneously live arena/scratch buffer
and every diagnostic/check block. Preflight checked products/sums and exact
fallible reservations before the corresponding allocations. Preserve release
count/emit consistency. Resource errors keep existing stages; allocation failure
is claimed only for real fallible allocation results, not as a catch-all.

Unit 1 itself introduces no allocation to fail. Existing declaration/plan
allocation-failure controls remain applicable. In later units prove inclusive
and one-over boundaries, zero/unit and many repeated structural types, lowered
limits for unreachable ceilings, and allocation failure per new reservation.
Requested payload excludes allocator overhead/capacity, LLVM/tool allocation,
OS internals and total RSS; no universal OOM-recovery promise is made.

## 9. Independent evidence and host gates

Unit 1 requires independent mathematical layout/type cases, every scalar and
length 0..1024, 1025/usize::MAX rejection, malformed/equal-malformed nominal
IDs, same-layout distinct types, nominal identity and retained representation
measurements. Existing record execution-plan tests must exercise the production
shared seam. Source array forms continue to reject and original scalar/record
and legacy controls remain consistent. Record focused commands, exact status
and counts, followed by proportional fmt/Clippy/debug/release ordinary gates.

Before activation use an independent scalar-sequence plus owner/capability
model. Enumerate lengths 0..4 with boundary index classes including i32 minima
and maxima; compose moves/replacement/generations/reborrows, joins/continue,
left-to-right arguments and RHS/index snapshots. Test every smaller fuel budget
for small traces and selected large boundaries, with fuel-before-bounds and
no-final-store assertions. Preserve earlier helper effects on final failure.

Raw mutations include element/index mismatch, wrong aggregate kind, mismatched
call/return lengths, missing/excess constructor elements, undominated index
locals, wrong-file/invalid origins, moved roots, staged-owner access and forged
permissions. Held-outs include zero/unit arrays, same type across files,
unavailable/pending-exclusive length, all owner classes, and bounds/arithmetic
splits before acyclic phis. A separate reviewer audits indexed memory access,
layout and accounting; a separate held-out source model supplies expectations.

The existing one-file frontend/reference behavior and ordinary test matrix cover
Linux x86_64, Windows x86_64, macOS x86_64 and macOS arm64. Linked typed-project
loading remains qualified on Linux x86_64 only; the existing non-Linux E0005
source-policy boundaries in RFC 0015 remain. Arrays do not broaden loader or
native hosts. Native remains Linux x86_64, LLVM 19.1.7,
O0 and the existing whole-project recursion/resource restrictions. Actual ELF
loads/stores, bounds dominance, source-free failures, guarded/acyclic paths and
debug/release parity are required native evidence. Sanitizers are supplementary.
Reuse applicable predecessor qualification; hosted exact-head CI is recorded
separately from local evidence. Nothing in a Unit 1 green test implies Unit 4
qualification or public array support.

## 10. Alternatives and decision

Slices/heap arrays or stored-reference lifetime work would add allocation and
escape contracts before this collection kernel needs them. Nominal synthetic
records would break cross-module structural identity and consume unrelated
record/field budgets. Packing arrays into spare RecordId bits would risk
reinterpreting malformed nominal IDs. A global type-intern table would introduce
allocation, lookup work and source-origin coupling without benefit for three
scalar types and bounded lengths. A small explicit checked descriptor avoids
those costs, while migration of executable carriers waits for Unit 2 review.

All ordinary grammar/layout choices above are concrete proposed defaults. Any
later change to them, limits or staged activation requires an explicit reviewed
RFC amendment and corresponding evidence; implementation alone is not acceptance.
