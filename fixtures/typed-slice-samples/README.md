# Call-only borrowed scalar slice sample

This experimental typed-preview example processes fixed i32 arrays of lengths
2, 3 and 0 through the same helpers across three declared modules:

- `main.ox` owns `[1, 2]`, `[3, 4, 5]` and an explicitly typed empty array
- `buffers.ox` increments each element with `bump`, then uses `relay` to pass an
  explicit exclusive reborrow to `bump` and a shared reborrow to `stats::sum`
- `stats.ox` sums any accepted length through a shared `&[i32]` parameter

The resulting arrays are `[2, 3]`, `[4, 5, 6]` and `[]`. Their sums are 5, 15
and 0, so `main` returns **515** (`5 * 100 + 15 + 0`). The empty view executes
neither loop body; an attempted element access would still fail bounds checking.
No array elements are copied merely to form or forward these views.

Run from the repository root with the current experimental compiler:

```sh
oxid check fixtures/typed-slice-samples/main.ox --edition typed-preview
oxid run fixtures/typed-slice-samples/main.ox --edition typed-preview
oxid compile fixtures/typed-slice-samples/main.ox --edition typed-preview \
  --backend llvm --output /tmp/oxid-slice-samples
/tmp/oxid-slice-samples
```

The reference run and a successfully compiled native program print `515`.
Use a fresh output path. Declared-child loading requires Linux. Native compilation
requires Linux x86_64 and LLVM/Clang/LLD 19.1.7 at O0 under the existing stricter
native admission limits, and retains the host ELF-loader/libc dependency.
These commands do not select a project-wide edition or require a manifest.

See [the slice contract](../../rfcs/0019-borrowed-scalar-slices.md),
[the typed-preview specification](../../spec/typed-preview.md#call-only-borrowed-scalar-slices)
and [public acceptance controls](../../tests/typed_slices.rs). Acceptance controls
do not establish exact-head hosted CI; historical source-bound evidence remains
scoped to its original compiler. This example adds no ranges, subslices, owned
unsized values, element references, heap collection or stability claim.
