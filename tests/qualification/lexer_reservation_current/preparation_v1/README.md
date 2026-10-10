# Fallible lexer reservation current-adapter preparation

Status: **inactive preparation, no successor generated**. These helpers do not
admit the compiler at the preparation checkout, and their Python tests are not a
compiler, Unit1/2/4, lifecycle, parser, native, or hosted qualification pass.

The approved design is
`docs/architecture/fallible-lexer-source-consumer-audit.md`, SHA-256
`8e5791d93dadfdf41f42c266ab9f1a2921317d6f60f0a9fd7bd6d0396a712caa`.
The preparation checkpoint is `4a82a9c`; it is **not the final source freeze**.
No private parser helper has been read or executed by this preparation.

## Contents and boundaries

- `adapters.py`: exact reversible Unit1 counter-domain, actual-core lifecycle,
  successful-append token, and retained budget-reserve observation transformations.
- `source_transition.py`: complete outer source/fixture/include admission. It has
  no generation side effect and requires an independently pinned final seal.
- `ci_inventory.py`: verifies all 173 approved direct workflow inventory entries.
- `unit1_lexer_controls.rs`: one additional current-only capacity/counter-error
  diagnostic control, including pending multi-byte token and EOF full spans.
- `test_preparation.py`: source-only helper mutation tests, never a Rust build.
- `GENERATION.md`: required freeze, generation, integration, and review sequence.

The existing 402db5 manifest, byte-storage helper, transition patch, authority,
original Unit1 bodies/rosters, Unit2 accounting authority, Unit4 public and parser
packages/maps, CI inputs, and all earlier inverse stages remain untouched.
There is deliberately no active dispatcher, successor manifest, authority,
transition patch, generated current/derived map, or final `seal.py` in this folder.

Run only the preparation controls without a build lease:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B tests/qualification/lexer_reservation_current/test_preparation.py -v
PYTHONDONTWRITEBYTECODE=1 python3 -O -B tests/qualification/lexer_reservation_current/test_preparation.py -v
```

The optimized-Python run verifies our explicit guards survive optimization. It
does not authorize optimization for retained controllers that explicitly refuse
it. No `__pycache__` directories may enter closed source or qualification roots.

## Independently source-derived Unit1 schedule

Frozen minimal fixture: root `mod a;\n` (7 bytes), child
`fn f()->(){return;}\n` (20 bytes). Ordinary counts are five and thirteen;
EOF occupies the last slot of each tape. The five current exact-capacity lexer
requests are global ordinals 5, 6, 19, 20, 21:

| Global ordinal | Requested slots | Source ID | Pending span |
| --- | --- | --- | --- |
| 5 | 4 | 0 | 0..3 |
| 6 | 8 | 0 | 6..7 |
| 19 | 4 | 1 | 0..2 |
| 20 | 8 | 1 | 5..6 |
| 21 | 16 | 1 | 10..11 |

Derivation from `project.rs`, `source.rs`, `project/filesystem.rs`, and the
closed ModuleCandidate path in `parser.rs`: source bytes, entry display, line
starts, source files; two root lexer requests; module declarations, module items,
file ASTs, module headers; logical module path, module display, module probe path;
child file ASTs, module headers, source bytes, line starts, source files; three
child lexer requests. These expectations precede candidate execution and have
not been inferred from successful current allocator traces.

The adaptation admits only exact label `lexer token tape`, exact ordinal/target/
element-width/success roster, E0400, stage `lex`, exact resource-limit text,
independent complete spans, empty secondary and notes. Every nonlexer stage/text
branch is retained. All original counter-prefix and primary-origin assertions
remain outside the branch, and the original 69 names remain separate from the
one new control. No ordinary diagnostic normalization is allowed.

Rust does not promise exact returned capacity. This is the frozen Linux x86_64
Unit1 fixture schedule on the prescribed toolchain, not a portable capacity
claim; overreturn/skipped-target qualification belongs to the compiler suite.

## Instrumentation meaning

Lifecycle attempt is inserted at `lex_core` entry before storage/recognition;
completion is after the successful EOF append. Neither `lex_with_limit` nor
`lex_with_allocator` gains a second hook. Direct test-only core calls and both
formatter parses each produce their own genuine core lifecycle. Provider
observation is not a core invocation; invalid association can still follow an
already executed canonical core, whose events must remain in receipts.

The token hook is inserted after the one `tokens.push(token)` and before its
successful return. Both ordinary and EOF sites call that helper. Any rejected
append exits before the push and hook. The retained budget hook stays immediately
before the actual trace push. The control parser build does not gain reserve or
token hooks. Historical `compose_division_lexer` remains unchanged for historical
instrumentation checks; its literal ordinary/EOF push adapter is never applied
to the new source.

Static seam/count/reversibility tests cannot prove runtime event cardinality,
full historical token-stream equality, or no subsequent parser/tool/output work.
Those acceptance gates remain required after the source/adapter freeze.
