# i32_algorithms: ordinary-source i32 algorithms

A pure `typed-preview` package over the existing nominal `vec::I32Vec` from
`packages/i32_vec`. It defines no container of its own, performs no I/O, and
adds no allocation, runtime primitive, or execution-policy knob.

## Import and public API

A repository project can depend on the package under any alias, for example:

```toml
[dependencies]
algorithms = "../../../packages/i32_algorithms"
```

```oxid
use algorithms::heapsort;
use algorithms::LowerBound;
use algorithms::lower_bound;
```

The complete public API is:

- `heapsort(values: &mut I32Vec) -> bool`: ascending, signed-i32,
  in-place, **unstable** heapsort. Returns `true` after success. Returns
  `false` immediately if an in-range `get` unexpectedly yields `Missing` or a
  `set` rejects an in-range index.
- `LowerBound::{Index(i32), ContractViolation}`: lower-bound result or an
  unexpected missing in-range element during search.
- `lower_bound(values: &I32Vec, key: i32) -> LowerBound`: for an already
  ascending vector, returns the first index with value `>= key`, or `len` if no
  such element exists. It does not sort, mutate, allocate, or validate
  sortedness. An unsorted input violates the precondition; its result is not
  specified as meaningful.

No other public helper or mutation API is provided. Both operations use only
scalar locals and existing `get`/`set`; neither creates another vector or
allocates an element payload. Heapsort uses `O(n)` element reads for an already
nondecreasing vector and otherwise retains its `O(n log n)` package-level
element-operation bound; lower-bound element reads/comparisons are `O(log n)`.
Both use `O(1)` scalar auxiliary storage and no recursive call stack. These are
package-level bounds, not claims about whole-runtime cost: existing vector
owner validation, bookkeeping, memory policy, and execution fuel still apply.

If an impossible contract violation occurs during sorting, earlier writes may
remain; there is no rollback or promised permutation after such a violation.
Likewise, a terminal runtime/fuel failure is not converted to `false`, and no
rollback is promised if it interrupts writes. On a successful ordinary call,
heapsort preserves length and the input multiset (it only swaps existing
values). The algorithm is not stable, so equal values have no relative-order
promise.

## Heapsort invariant and termination

The algorithm uses a max heap over the half-open range `[0, end)`. For each
parent, children have indexes `2*root+1` and `2*root+2` when those indexes are
inside the range. The max-heap property is that every parent is at least as
large as either existing child under signed `i32` comparison.

**Build phase.** `parent` starts at `n/2`, one past the final possible
non-leaf. Each iteration decrements it before sifting, so the parents are
visited from `n/2-1` down through `0`. Leaves already satisfy the heap
property; when a parent is sifted, its child subtrees have already been
heapified. Sift-down repeatedly selects the larger existing child and swaps it
with the root only when that child is larger. The displaced parent continues
at the selected child's index. Thus the affected path is repaired, while the
other subtree remains a heap. At termination the whole `[0,n)` is a max heap.

**Extraction phase.** For `end=n,n-1,...,2`, `end` is decremented first, the
maximum at root `0` is swapped with index `end`, and sift-down repairs only
`[0,end)`. The removed maximum is now at the start of the sorted suffix. It is
at least every remaining heap element, and successive extracted maxima are
nonincreasing in extraction order, so the final suffix is ascending and the
prefix/suffix partition remains valid. Each iteration reduces `end`; each
sift iteration moves `root` to a strictly larger child index, and a heap path
is finite. Both loops terminate. `n < 2` returns before any `n-1`, swap, or
sift operation, leaving empty and singleton vectors unchanged.

**Nondecreasing-input scan.** After the `n < 2` return, heapsort reads index
`0` once into `previous`, then scans indexes `1` through `n-1` in order. At
each step, `previous` is the value at the immediately preceding index. A
`Missing` result returns `false`; if `previous > current`, the scan stops and
the original complete heap-sort path runs unchanged. Otherwise `current`
becomes the next `previous`. Reaching the end proves every adjacent pair is
nondecreasing, so the call returns `true` without writes or allocation. The
scan is bounded by `n-1` subsequent reads and terminates by increasing its
index toward `n`. Each read has `1 <= index < n`, and incrementing gives
`index + 1 <= n`, so both access and addition are safe; the guarded `n >= 2`
also makes the initial index `0` valid. A sorted vector takes `O(n)` reads. An inversion
does not change the unsorted path's `O(n log n)` package-level operation bound.
This scan does not promise that arbitrary inputs fit any particular fuel
budget.

**Child-bound proof, including odd/even sizes.** Sift-down only tests the parent
condition when `end >= 2`, using

```text
root <= (end - 2) / 2
```

For nonnegative indexes this is equivalent to `root < end/2`. It establishes
`2*root+1 <= end-1`, so the left child always exists. The right index is
considered only if `left+1 < end`. With an even heap size, the final internal
parent can have just its left child (for example `end=4`, `root=1`, left `3`,
right `4`); the conditional excludes the absent right child. With an odd size,
the final parent has two children (for example `end=5`, `root=1`, children `3`
and `4`), and both are compared so the larger is selected. At `end=2`, root
`0` has exactly child `1`; at `end=3`, root `0` has children `1` and `2`.
The condition does not include a leaf. The superficially similar
`root <= (end-1)/2` is deliberately not used because for odd `end` it can
include the last leaf and produce an out-of-range left child.

**Arithmetic bounds.** The existing `I32Vec` represents at most
536,870,911 elements (its initialized byte length is a multiple of four no
larger than 2,147,483,644). All algorithm indexes are nonnegative and at most
`n`. Under the guarded parent condition, `2*root <= end-2`, hence
`left=2*root+1 <= end-1`; therefore `left+1 <= end`, and adding one cannot
overflow. The `right` get is restricted to `right < end`. Heap ends and parent
indexes only decrease by one while positive; no subtraction is performed on a
negative index. The guarded expression `end-2` is evaluated only for
`end>=2`. The extract loop decrements only when `end>1`. These bounds also hold
for the more general nonnegative signed-i32 length limit, not merely the
current smaller representable vector bound.

Values are compared directly with `<`, `>`, and `<=`; no element value is
negated or subtracted. Thus both `-2147483648` and `2147483647`, duplicates,
and all other signed values need no special arithmetic case.

## Lower-bound invariant and arithmetic

The search maintains `0 <= lo <= hi <= n`. At each iteration `lo < hi`, so
`hi-lo` is positive and

```text
mid = lo + (hi-lo)/2
```

satisfies `lo <= mid < hi <= n`; the midpoint is an in-range index. If
`values[mid] < key`, sortedness implies every index through `mid` is too small,
so `lo=mid+1` remains valid. Otherwise `mid` is a candidate and sortedness
implies no later element can be the first candidate, so `hi=mid`. This keeps
the first qualifying index, including when duplicates equal `key` occur. Each
branch strictly shrinks `[lo,hi)`, proving termination. When `lo==hi`, all
indexes below it have value `< key`, and all indexes from it onward (if any)
have value `>= key`; therefore `lo` is the first qualifying index or `n`.
The empty range returns `Index(0)` without a read.

The midpoint expression does not add `lo+hi`. Since `mid<hi<=n`, `mid+1<=hi`
in the increment branch, and every search index stays representable. Values
are compared directly, never subtracted, so MIN/MAX are safe. An unexpected
`Missing` at this proven in-range midpoint returns `ContractViolation` rather
than inventing an index; sortedness is a caller precondition, not runtime
validation.

## Validation scope

At `d34333e`, focused independent-client acceptance passed CHECK, typed format
checks, exact interpreter result `42\n`, and all nine named contracts, but the
original CLI acceptance passed eight cases and then failed on `sorted-259` with
`error[E0601]` execution fuel exhaustion in the existing `i32_vec` encoder.
That historical failure was in `encode_le` via `set` writes, not in `get`;
insertion sort was linear on already-sorted input, while heapsort performed
many swaps. No fuel, corpus, assertion, or runtime change was made. Exact
validation evidence for the separate correction is reported separately by the
tree; this README does not claim that correction validation passes. The
repository's historic native `oxi32sort` evidence belongs to the previous
app-local insertion sort and does **not** validate this heapsort or package;
native compatibility remains unvalidated. Benchmark improvement,
whole-runtime asymptotics, allocation-failure injection, and fuel/capacity
qualification are not claimed.
