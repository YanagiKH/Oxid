# Fixed-array Unit2E producer-native CI admission

This producer adapter admits the existing ignored owned-native prefix. Its six
array tests remain producer assertions. Their 7,058 ELF executions per profile
are asserted in the original test summaries; the 6,793 reference comparisons
are derived from the source control flow. They are not per-process receipts or
independent replay evidence.

The initial implementation checkpoint provides strict admission functions and
synthetic controls. It does not yet expose operational `run` or `package`
commands or change CI. Process/tool/source admission, stable packaging and CI
wiring are the next implementation step. Independent final-receipt integration
and the separate current-source binding refresh remain required before complete
Unit2E qualification.

The test roster is exactly 16 names, including six array families. The stdout
parser attributes each successful completion by name and preserves its original
byte and line span. The source-resource test alone uses its exact three-line
stdout form. Original stderr must contain each of the six full family summaries
exactly once. Neither a footer nor success-looking text substitutes for actual
child status in the forthcoming process controller.

Each profile requires 1,307 unique array artifact names: 524 executable ELF64
little-endian x86_64 files, 524 compiled UTF-8 LLVM modules, and 259 original
guarded production LLVM modules. Original inherited artifacts are retained
separately by role. The adapter rejects missing, extra, nested or symlink array
members and records actual bytes, modes and SHA-256 values.

Run only the synthetic boundary controls with:

```sh
python3 -B -m unittest discover -s scripts -p test_owned_array_native.py -v
```

These tests execute no Cargo, LLVM or native corpus and cannot qualify a real
producer run. The production Rust tests, source-binding authorities and existing
array gates are unchanged.
