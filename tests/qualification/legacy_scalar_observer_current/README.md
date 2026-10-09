# Current-source compatibility for frozen scalar observers

The bounded parser and static proof builders copy the current canonical frontend
unchanged. The u8 frontend added two required source modules:
`parser/conversions.rs` and `declaration_index/u8_reservation.rs`. Both are now
explicit members of each builder's source closure, hashed with the other copied
canonical files. Omitting either is tested by an actual failed rustc build.

The historical observer fixtures remain byte-for-byte unchanged. The explicit
`scripts/legacy_scalar_observer_u8.py` compatibility successor accepts only the
seven named, hash-pinned wrapper inputs and applies exact, one-occurrence,
reversible edits. It also pins all seven derived outputs. Unknown wrappers,
changed old bytes, altered edits, and nonreversible adaptations fail closed.
Canonical source patches remain empty. The source manifest records original and
derived wrapper hashes and the compatibility schema; the bounded proof verifier
reconstructs each derived wrapper rather than trusting a self-reported hash.
The retained wrapper-only diff shows all compiled wrapper bytes.

## Scope of the legacy domains

- The parser still projects unqualified type names without interpreting them.
  Thus a name spelled `u8`, like any other type name, remains a complete syntax
  observation. The new `Conversion` AST form is explicitly `outside_subset`;
  its unreachable projection arm prevents accidental partial observations.
- The static observer excludes the new conversion form before resolution. It
  also excludes the actual `u8` parameter, result, and local annotation type
  nodes, including nested blocks, before resolution or typed projection. This
  prevents emitting `u8` facts under the frozen scalar schema. Ordinary values
  and functions named `u8`, comments, and existing unknown-type route selection
  are unaffected. Resolve-only mode has the same boundary.
- New enum variants remain exhaustiveness errors rather than wildcard matches.
  The HIR conversion projection arm is unreachable after the AST boundary.
- No historical corpus, projection schema, canonical diagnostic, capacity,
  parser, checker, source oracle, or native implementation is changed.

Run the live build, boundary, omission, and coherent-tampering controls:

```sh
python3 -B -m unittest discover -s scripts -p test_legacy_scalar_observer_u8.py -v
python3 -O -B -m unittest discover -s scripts -p test_legacy_scalar_observer_u8.py -v
```

These tests run in the existing repository-wide script discovery. The unchanged
CI parser/static steps still run all historical projection tests and complete
reference/native proof matrices in both ordinary profiles. Local evidence uses
real LLVM tools and does not stand in for hosted platform or package staging.
