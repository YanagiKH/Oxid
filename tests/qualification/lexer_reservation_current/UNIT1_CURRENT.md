# Current fallible-lexer Unit1 runner

`unit1_run_current.py` is the separately named current runner. It retains the
original CLI (`--repo`, `--profile debug|release|both`, `--output`, `--target-dir`,
`--cargo`) and defaults to both debug and release profiles. This generated package
is source-only work; no Rust build or Unit1 qualification result is asserted.
Execution requires the coordinator's subsequent source/adapter acceptance and
serial build authorization.

The original package in `tests/fixtures/typed_project_unit1_independent` remains
byte-identical. `unit1_original_package.json` pins every one of its 16 files,
including both original runners, the complete frozen fixtures, original resource
expectations, provenance, and exact 69-name roster, with full bytes/hash/Git blob
and mode. The current support authenticates the closed membership before any
staging or helper execution.

## Exact current derivation

- `unit1_derivation.py` contains explicit single-occurrence reversible seams for
  the original runner and libtest controller. Both complete original bodies are
  recovered and compared. Their guards and fixture/resource expectations remain.
- `unit1_reviewer_additional.current.rs` is the full exact output of the approved
  `adapters.unit1_counter_domain`. Its full inverse must recover the frozen body.
- `unit1_correspondence.json` binds both reviewer bodies, both runner derivations,
  every original case identity, and the separately counted current-only control.
- `unit1_current_bindings.json` pins the complete current bodies and recipe. The
  support pins this map and the complete original-package map before loading any
  executable helper. Existing bytecode caches are never executed by that loader.
- All 16 frozen files are staged unchanged first. Only staged
  `reviewer_additional.rs` is replaced. The new libtest controller has its own name;
  staged original `run.py` and `run_reviewer.py` remain unchanged. The one new
  control is included separately from the original fixtures.

The active public source-binding `preflight(repo, package=PACKAGE)` runs before
source materialization, against the isolated source before the review include,
and before and after each profile against the supplied source. Its exact current
manifest is 74,327 bytes, SHA-256
`aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633`, for compiler
checkpoint `c8e9a72afd9866f32b96f98ae24f61390039f421`. The trusted dispatcher comes
from this runner's checkout; the candidate repository contributes data only.
Historical inverse bodies are never used as current compiler input.

The original before/after resource inventory, supplied/compiled source hashes,
executable identity, per-test timeouts, exact libtest listing, and precisely one
named nonignored passing execution checks are retained. Added profile guards
also authenticate the staged original/current helpers and frozen roster before
and after execution. The controller compares the frozen roster hash again after
execution.

## Accounting

Each profile requires exactly 69 original case names and exactly one separately
identified current control:
`frontend::project::reviewer_unit1::current_lexer_storage_failure_full_span_and_eof`.
All 69 original result rows remain in `tests`; `passed`, `total`, and `expected`
remain original-only counts. `current_controls` has separate rows and counts;
`all_*` reports the combined 70. Neither a missing control nor a replacement of
an original case can pass. The top-level `test_functions_executed` stays 138 for
both profiles, and `current_control_test_functions_executed` is separately 2.
A failed new control fails the profile even if all 69 originals pass.

## Source-only controls

Run with no compiler or fixture generation:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B tests/qualification/lexer_reservation_current/test_unit1_current.py -v
PYTHONDONTWRITEBYTECODE=1 python3 -O -B tests/qualification/lexer_reservation_current/test_unit1_current.py -v
```

These controls cover full forward/inverse bodies, every derivation seam, original
and derived body/map mutation, extra/missing originals, modes/symlinks, one-body
staging, separate include identity, active-stage admission, retained guard order,
and a Python fake-libtest protocol with missing/duplicate/substituted rosters,
zero/ignored/wrong-name/nonzero execution, control-only failure, and changed
executable/roster during execution. They are not Rust execution evidence. Both
production controllers still refuse optimized Python, preserving the original
assertion guard boundary.
