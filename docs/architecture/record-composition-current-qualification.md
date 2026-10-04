# Bounded record composition current qualification

This successor binds source checkpoint `8ae66ef5543bcb1251868b84ea38a82c2649a3a4`
(tree `f6b7dee8bac4ebcc27ad020db9940344c5e4ae41`) without changing its compiler.
The exact five-file slice native qualification fix published as
`e4bc78b14bcd088246cb883d3608757f899c5c2d` is the integration predecessor.
Historical qualification remains historical. Neither adapter admission nor old
receipts establish current semantic, resource, or native qualification.

## Closed source and sample inputs

`typed_project_source_binding/current-source.json` has 196 inputs, including
140 compiler source members (143 bodies with Cargo manifests/build script).
The composition transition contains 40 paths: 32 changed bound source members,
four new Rust modules, and the public integration test with its three literal
sample inputs. The public sample includer is distinct from the unchanged
47-reference/42-input array fixture includer. Exact inverse patching restores all
188 slice inputs before the existing division/combined/formatter/source chain.
The prior manifest is retained byte-for-byte as `slices-source.json`.

The current parser successor admits 359 base and 362 instrumented/control
members. Its two composition-overlap paths, AST and parser, are reversed to the
exact slice predecessors before older transformations are checked. Current
instrumentation is applied to the actual composition bodies; historical bodies
and observation expectations are not substituted for current execution.

## Explicit semantic amendments

Only these composition changes supersede the named historical expectations:

- `owned_source/nested-record-field`: an acyclic record-valued field is now
  accepted; the unchanged `main` returns i32 zero
- `owned_source/excluded-chained-projection`: `x.value.other` now parses. Because
  `value` has scalar i32 type, typing rejects with E0305 at `other`, bytes75..80,
  with “intermediate field access requires a record”. The historical parse
  E0101 at the second dot, bytes74..75, is retained as the predecessor fact
- `fixed_array_source_unit3/contracts-v2/excluded-array-record-field`: a fixed
  scalar array record field is now accepted; the unchanged main returns zero.
  Its original `FROZEN_EXPECTATION_NOT_EXECUTED` status is retained. Current
  acceptance does not retrospectively execute or qualify that historical row

The sealed opt-in `owned-record-composition-v1` CLI amendment keeps the two
checked-division amendment rows unchanged and adds exactly the two owned-source
rows above. It applies only to explicit current CLI mode, never candidate/raw
traces, native fuel expectations, or historical files. The separate array row is
recorded in `record-composition-semantic-amendment-v1.json`; it is not silently
inserted into an unrelated corpus. Formatter's accepted composition array-field
case was already introduced in the source checkpoint.

## Resources

The independently derived resource appendix is in
`tests/qualification/record_composition_resource_v1/`. Original evidence and
ceilings are unchanged. Representation increases include declaration tables,
HIR fields/projections and fixed observer auxiliary storage. In particular,
None projection slots also grow, and unchanged scalar programs can have changed
requested-byte accounting. A source-bound rerun of the named retained layout
checks, both profiles, is required in addition to the standalone declaration
probes. Scalar-only resource redundancy arguments do not generalize to composed
record DAGs.

## Native discovery and execution

Closed current ignored-prefix discovery is exactly24: original16 + slice3 +
composition5. The producer still explicitly selects and runs the original16
with `--exact`; new tests cannot substitute for historical qualification.
Dedicated CI commands execute all slice/composition ignored tests in both
profiles. The six-substitution `BorrowedSlot` Unit2D adapter remains unchanged;
its predecessor, derived hash and exact reverse controls retain authority.

## Evidence status

Admission controls and source-only preparation are prerequisite checks. Actual
producer execution, independent Unit2D debug/release, Unit2/parser/public/CLI
replay, resource checks and exact-head hosted checks must be recorded separately
before describing the successor as qualified. This document does not imply
completion of M2/M3, memory-safety proof, Rust compatibility, or v1.0.
