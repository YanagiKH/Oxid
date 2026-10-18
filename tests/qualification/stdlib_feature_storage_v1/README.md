# Stdlib historical storage successor

candidate.py and derive.py retain their reviewed semantics and exact bytes. The
four feature before bodies and originals.json remain unchanged.

archive/test_storage.py.data preserves the original unpublished test verbatim.
storage_test_derivation.py pins those bytes and derives the discoverable test
with exact replacement seams: one checkout-copy route, tracked/untracked controls,
and an isolated historical baseline restored from four independent literal pins.
All original rejection and complete-view identity assertions remain present.
The derivation itself is tested; no test is skipped. The manifest describes the
current package, not the superseded external ten-file review delivery.

Run from the repository root:

    python3 -B -m unittest discover -s tests/qualification/stdlib_feature_storage_v1 -v
    python3 -O -B -m unittest discover -s tests/qualification/stdlib_feature_storage_v1 -v

Optimized mode preserves the existing historical-consumer refusal assertion.
