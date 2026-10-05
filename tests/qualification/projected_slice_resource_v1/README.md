# Projected array-slice physical resource successor

This separately named successor preserves the complete record-composition resource
package and its historical numbers. The source identity and independent derivation
are in ledger.json and independent-review.md. No cap is raised.

Reference handles/loans occupy80/112 bytes (10/14 physical cells), while unchanged
source keeps8/12 logical activation-fuel cells. OIR paths and sparse typed paths
are real retained payload and are charged separately. The200000/200001 and8192/8193
fixtures explicitly reduce padding to retain exact physical boundaries.

Run python3 -B -m unittest discover -s scripts -p test_projected_resource_contracts.py.
Run current source tests containing source_resource, reviewer_source_resource,
projected_slice_retained_source_layouts, aggregate_seam_retains_raw,
projection_payload_admission and projected_ in debug/release. With LLVM19 run
source_native_actual_slot_and_cell_boundaries_use_real_llvm --ignored in both
profiles. Frozen prior facts are not reclassified as current execution.
