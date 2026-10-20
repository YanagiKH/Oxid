# oxi32sort: little-endian i32 record sort Process CLI

Experimental ordinary-source workspace for the fixed-record sorting CLI. The
app depends on the existing `packages/i32_vec` package (`vec`) and the pure
`packages/i32_algorithms` package (`algorithms`). The latter operates directly
on the same nominal `I32Vec`; it does not redeclare or copy the type. The
algorithm package performs no I/O. Only `oxi32sort` reads and writes process
streams.

```text
oxi32sort/
  oxid.toml                 [workspace] members = ["app"]
  app/oxid.toml             project "oxi32sort", entry src/main.ox,
                            vec = "../../../../packages/i32_vec",
                            algorithms = "../../../../packages/i32_algorithms"
  app/src/main.ox           Process-mode CLI; the only member that does I/O
  cases/<name>.input.bin    fixed binary oracle inputs
  cases/<name>.expected.bin fixed expected stdout (empty for exit 65)
```

The app imports `algorithms::heapsort` and calls it after complete input
conversion. It no longer contains a local sorting implementation. The shared
algorithm library documents the heap invariant, arithmetic bounds, failure
semantics and unstable ordering in `packages/i32_algorithms/README.md`.

## Application behavior

```sh
oxid run --manifest-path examples/projects/oxi32sort/oxid.toml --package oxi32sort --engine interpreter --entry-mode process < input.bin > output.bin
```

The app reads stdin to EOF with `read_some` into an initialized 1,024-byte
chunk, appending each chunk to one growable `ByteBuf`. `from_le_bytes` then
converts the complete input; any length that is not a multiple of four is
`Invalid`. The resulting `I32Vec` is sorted in place by `algorithms::heapsort`
into ascending signed-i32 order. On success, the vector is consumed with
`into_le_bytes`, recovering one `ByteBuf` without a second payload copy, and
the unchanged `write_some` loop writes it to stdout while handling partial
progress. Linux EINTR (4) is retried on both read and write; a write reporting
zero bytes is rejected. Sorting completes before output begins.

The heapsort is **not stable**: equal records have no relative-order promise.
For this CLI, records are just four-byte i32 values, so its externally visible
success contract is the exact sorted multiset encoding, not record identity or
stability.

| Outcome | Exit | Stdout |
| --- | --- | --- |
| Whole records (including empty input) | 0 | the same records, sorted ascending as signed i32, little-endian |
| Trailing 1, 2 or 3 bytes after the last whole record | 65 | empty |
| Capacity overflow (read buffer initialization or growth) | 70 | empty |
| Algorithm contract violation during sort (`get` returned `Missing` or `set` returned false for an in-range index) | 70 | empty |
| Allocation failure or memory-limit policy refusal during read buffer initialization or growth | 71 | empty |
| Read/write error or zero-progress write | 74 | possibly a prefix |

A malformed tail never produces output because conversion and sorting complete
before any write. A write failure can occur after part of the output was
written; no rollback is promised. This is a complete-buffer program, not
streaming: the whole input is held. Heapsort first scans for nondecreasing
input, taking `O(n)` package-level element reads when the records are already
sorted; if it finds an inversion, it uses the unchanged `O(n log n)`
package-level heap path. It uses `O(1)` scalar auxiliary storage and no
recursive call stack; do not read these as whole-runtime complexity or
owner-validation cost guarantees. The 1,024-byte read chunk, per-allocation
headers, capacity slack and growth's old-plus-new peaks still matter while
reading. Runtime limits and ordinary owner-return checks still apply; no input
size is promised to fit existing fuel or memory policy. Exit codes 70, 71 and
74 are documented paths, not claims of failure injection.

## Fixed binary oracle

| Name | Class | Input bytes | Exit | Stdout |
| --- | --- | --- | --- | --- |
| empty | empty input | 0 | 0 | empty |
| min-singleton | one record, i32 MIN | 4 | 0 | identical |
| max-singleton | one record, i32 MAX | 4 | 0 | identical |
| mixed-duplicates | 14 records, positive/negative, MIN/MAX, duplicates | 56 | 0 | sorted |
| short-reverse | 6 records strictly descending (3..-2) | 24 | 0 | sorted |
| tail-1 | 3 records + 1 byte | 13 | 65 | empty |
| tail-2 | 3 records + 2 bytes | 14 | 65 | empty |
| tail-3 | 3 records + 3 bytes | 15 | 65 | empty |
| sorted-259 | 259 ascending records crossing the 1,024-byte chunk | 1036 | 0 | identical |

The existing fixture files and pins in `tests/owned_i32_vec_package.rs` are
unchanged. That test independently re-derives each expected output (decode,
sort, encode) and requires every malformed tail to expect empty stdout.

## Validation and evidence boundary

The repository README and retained records describe **historic native and
interpreter runs of the previous insertion-sort app**, not this migrated
heapsort binary. In particular, the older native build/package/extracted
executable records cannot establish native compatibility of the new algorithm
package or current CLI. At `d34333e`, focused interpreter validation passed the
eight cases through `tail-3`, but `sorted-259` failed with `error[E0601]
(oir-run): execution fuel exhausted` in the existing `i32_vec` encoder. The
failure was in `encode_le` via `set` writes, not in `get`: insertion sort was
linear on already-sorted input, while heapsort performed many swaps. Work
stopped without raising fuel, changing the corpus, or weakening assertions.
The separate algorithm client at that tree passed all nine named contracts
and returned exactly `42\n`; this does not supersede the CLI failure. Exact
validation evidence for the separate correction is reported separately by the
tree; this README does not claim that correction validation passes. Native
compatibility remains unvalidated. No benchmark improvement,
allocation-failure injection, capacity/fuel qualification, forced short-write
or EINTR qualification, or full repository CI result is claimed here. This is
not a claim of G6/R2 completion.
