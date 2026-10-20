# i32 algorithms independent client contracts

This ordinary-source `typed-preview` client depends on the existing `vec` package and the separate `algorithms` package. It exercises only their public client APIs: `heapsort`, `lower_bound`, and `LowerBound`. The cases do not implement algorithm helpers, infer expected output from the functions under test, or introduce a replacement vector type.

The nine root cases are fixed and named in the source:

1. `test_sort_empty_and_singleton`
2. `test_sort_pairs_and_odd_even_heaps`
3. `test_sort_signed_extremes_and_duplicates`
4. `test_sort_sorted_reverse_equal_and_idempotent`
5. `test_sort_preserves_capacity_and_owner`
6. `test_lower_bound_empty_singleton_and_edges`
7. `test_lower_bound_duplicates_and_gaps`
8. `test_lower_bound_read_only`
9. `test_sort_then_search_client_composition`

The assertions cover hand-written exact sorted vectors and lower-bound indices, signed extremes, duplicate first-occurrence behavior, repeated-sort idempotence, read-only repeated searches, and sort/search composition. Within the existing `test_sort_sorted_reverse_equal_and_idempotent` case, the late-inversion input `[-3, -2, -1, 0, 1, 3, 2]` is checked against `[-3, -2, -1, 0, 1, 2, 3]`, and `[MIN, MIN, -1, 0, 0, MAX, MAX]` is checked unchanged; each is sorted twice with exact outputs checked both times. The owner case reserves requested capacity 37, records the actual capacity, and checks fixed little-endian output bytes for `[2, -1, 2]`; it also sorts a reserved empty owner. Every vector push and byte-buffer reserve/extend status is checked. The minimum i32 is written as `-2147483647 - 1`.

`main` runs the same nine named cases and returns the first distinct nonzero failure code, or 42 only after all cases pass. At `d34333e`, focused acceptance passed: CHECK succeeded, both sources passed typed format checks, interpreter result-mode RUN produced exactly `42\n`, and TEST reported all nine cases PASS. The separate original CLI acceptance stopped on `sorted-259` with `error[E0601]` execution fuel exhaustion in the existing `i32_vec` encoder; the failure was in `encode_le` via `set` writes, not in `get`. Insertion sort had been linear on already-sorted input, whereas heapsort performed many swaps. Exact validation evidence for the separate correction is reported separately by the tree; this README does not claim that correction validation passes. Native compatibility remains unvalidated.
